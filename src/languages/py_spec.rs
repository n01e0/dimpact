use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::util::{byte_to_line, line_offsets};
use crate::ts_core::{QueryRunner, compile_queries_python, load_python_spec};

pub struct SpecPyAnalyzer {
    queries: crate::ts_core::CompiledQueries,
    runner: QueryRunner,
}

impl SpecPyAnalyzer {
    pub fn new() -> Self {
        let spec = load_python_spec();
        let queries = compile_queries_python(&spec).expect("compile python queries");
        let runner = QueryRunner::new_python();
        Self { queries, runner }
    }
}

impl Default for SpecPyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for SpecPyAnalyzer {
    fn language(&self) -> &'static str {
        "python"
    }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let offs = line_offsets(source);
        let lines: Vec<&str> = source.lines().collect();

        #[derive(Clone)]
        struct Decl {
            name: String,
            decl_kind: String,
            start_line: u32,
            end_line: u32,
        }

        let mut decls = Vec::<Decl>::new();
        for caps in self.runner.run_captures(source, &self.queries.decl) {
            let Some(name_cap) = caps.iter().find(|c| c.name == "name") else {
                continue;
            };
            let Some(decl_cap) = caps.iter().find(|c| c.name == "decl") else {
                continue;
            };

            let name = source[name_cap.start..name_cap.end].trim();
            if name.is_empty() {
                continue;
            }

            let sl = byte_to_line(&offs, decl_cap.start);
            let start_idx = (sl.saturating_sub(1)) as usize;
            let end_line = if start_idx < lines.len() {
                find_python_block_end(&lines, start_idx) as u32
            } else {
                sl
            }
            .max(sl);

            decls.push(Decl {
                name: name.to_string(),
                decl_kind: decl_cap.kind.clone(),
                start_line: sl,
                end_line,
            });
        }

        let class_ranges: Vec<(u32, u32)> = decls
            .iter()
            .filter(|d| d.decl_kind == "class_definition")
            .map(|d| (d.start_line, d.end_line))
            .collect();

        decls
            .into_iter()
            .map(|d| {
                let kind = match d.decl_kind.as_str() {
                    "class_definition" => SymbolKind::Struct,
                    "function_definition" => {
                        if class_ranges
                            .iter()
                            .any(|(s, e)| d.start_line > *s && d.start_line <= *e)
                        {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        }
                    }
                    _ => SymbolKind::Function,
                };

                Symbol {
                    id: SymbolId::new("python", path, &kind, &d.name, d.start_line),
                    name: d.name,
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: d.start_line,
                        end_line: d.end_line,
                    },
                    language: "python".to_string(),
                }
            })
            .collect()
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        use regex::Regex;
        use std::collections::HashSet;

        let offs = line_offsets(source);
        let mut out = Vec::new();
        let mut seen: HashSet<(u32, String, Option<String>, bool)> = HashSet::new();
        let imports = self.imports_in_file(path, source);
        let import_aliases: HashSet<String> = imports.keys().cloned().collect();
        let lines: Vec<&str> = source.lines().collect();

        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let Some(name_cap) = caps.iter().find(|c| c.name == "name") else {
                continue;
            };
            let name = source[name_cap.start..name_cap.end].trim();
            if name.is_empty() {
                continue;
            }

            let qual = caps
                .iter()
                .find(|c| c.name == "qual")
                .map(|q| source[q.start..q.end].trim().replace([' ', '\t', '\n'], ""))
                .filter(|q| !q.is_empty());
            let call_cap = caps.iter().find(|c| c.name == "call");
            let ln = if let Some(c) = call_cap {
                byte_to_line(&offs, c.start)
            } else {
                byte_to_line(&offs, name_cap.start)
            };

            let is_method = if let Some(q) = qual.as_deref() {
                let first = q.split('.').next().unwrap_or("");
                !import_aliases.contains(first)
            } else {
                false
            };
            let key = (ln, name.to_string(), qual.clone(), is_method);
            if !seen.insert(key) {
                continue;
            }

            out.push(UnresolvedRef {
                name: name.to_string(),
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: qual,
                is_method,
            });
        }

        // Conservative dynamic edge from getattr/setattr string targets.
        // Example:
        //   getattr(obj, "build")(...) -> build
        //   setattr(self, "handler", ...) -> handler
        let re_assign_str = Regex::new(
            r#"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*[\"']([A-Za-z_][A-Za-z0-9_]*)[\"']\s*$"#,
        )
        .expect("valid python assignment string regex");
        let mut assigned_string: std::collections::HashMap<String, Vec<(u32, String)>> =
            std::collections::HashMap::new();
        for cap in re_assign_str.captures_iter(source) {
            let Some(full) = cap.get(0) else {
                continue;
            };
            let Some(var) = cap.get(1) else {
                continue;
            };
            let Some(val) = cap.get(2) else {
                continue;
            };
            let ln = byte_to_line(&offs, full.start());
            assigned_string
                .entry(var.as_str().to_string())
                .or_default()
                .push((ln, val.as_str().to_string()));
        }
        for entries in assigned_string.values_mut() {
            entries.sort_by_key(|(ln, _)| *ln);
        }

        let re_dyn_attr = Regex::new(
            r#"(?m)(getattr|setattr)\s*\(\s*([^,\n\)]+)\s*,\s*([\"'][A-Za-z_][A-Za-z0-9_]*[\"']|[A-Za-z_][A-Za-z0-9_]*)"#,
        )
        .expect("valid python getattr/setattr regex");
        let re_str_lit =
            Regex::new(r#"^[\"']([A-Za-z_][A-Za-z0-9_]*)[\"']$"#).expect("valid py str lit regex");
        let re_ident = Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*)$"#).expect("valid py ident regex");

        for cap in re_dyn_attr.captures_iter(source) {
            let Some(full) = cap.get(0) else {
                continue;
            };
            let Some(obj) = cap.get(2) else {
                continue;
            };
            let Some(arg) = cap.get(3) else {
                continue;
            };

            let ln = byte_to_line(&offs, full.start());
            let objq = obj.as_str().trim().replace([' ', '\t', '\n'], "");
            let qual = if objq.is_empty() { None } else { Some(objq) };

            let arg_raw = arg.as_str().trim();
            let resolved = if let Some(c) = re_str_lit.captures(arg_raw) {
                c.get(1).map(|m| m.as_str().to_string())
            } else if let Some(c) = re_ident.captures(arg_raw) {
                let var = c.get(1).map(|m| m.as_str()).unwrap_or("");
                assigned_string.get(var).and_then(|vals| {
                    vals.iter()
                        .rev()
                        .find(|(line, _)| *line <= ln)
                        .map(|(_, v)| v.clone())
                })
            } else {
                None
            };

            let Some(name) = resolved else {
                continue;
            };
            let is_method = true;
            let key = (ln, name.clone(), qual.clone(), is_method);
            if !seen.insert(key) {
                continue;
            }
            out.push(UnresolvedRef {
                name,
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: qual,
                is_method,
            });
        }

        // Decorator references can appear without explicit call syntax (e.g. @traced).
        // Treat decorator refs as non-method call-like unresolved refs for dependency edges.
        let re_decorator = Regex::new(
            r"(?m)^[ \t]*@([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)(?:\([^\n]*\))?",
        )
        .expect("valid python decorator regex");

        for caps in re_decorator.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(chain_cap) = caps.get(1) else {
                continue;
            };

            let chain = chain_cap.as_str().trim();
            if chain.is_empty() {
                continue;
            }

            let decorator_ln = byte_to_line(&offs, full.start());
            let mut ln = decorator_ln;
            let dec_idx = decorator_ln.saturating_sub(1) as usize;
            for i in dec_idx + 1..lines.len() {
                let t = lines[i].trim();
                if t.is_empty() || t.starts_with('#') || t.starts_with('@') {
                    continue;
                }
                if t.starts_with("def ") || t.starts_with("async def ") || t.starts_with("class ") {
                    ln = (i as u32) + 1;
                }
                break;
            }

            let compact = chain.replace([' ', '\t', '\n'], "");
            let mut parts: Vec<&str> = compact.split('.').collect();
            if parts.is_empty() {
                continue;
            }

            let name = parts.pop().unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            let qual = if parts.is_empty() {
                None
            } else {
                Some(parts.join("."))
            };

            let is_method = false;
            let key = (ln, name.to_string(), qual.clone(), is_method);
            if !seen.insert(key) {
                continue;
            }

            out.push(UnresolvedRef {
                name: name.to_string(),
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: qual,
                is_method,
            });
        }

        // Conservative descriptor edge:
        // If a class attribute is initialized with a descriptor class instance and that class
        // defines `__get__`, add a fallback edge from `self.<attr>(...)` callsites to
        // `<DescriptorClass>.__get__`.
        let re_class =
            Regex::new(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)\b").expect("valid class regex");
        let re_def_get = Regex::new(r"^\s*def\s+__get__\b").expect("valid __get__ regex");
        let re_class_attr_ctor =
            Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(")
                .expect("valid class attribute ctor regex");

        let mut descriptor_classes: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for i in 0..lines.len() {
            let line = lines[i];
            let Some(cap) = re_class.captures(line) else {
                continue;
            };
            let Some(class_name) = cap.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };
            let end = find_python_block_end(&lines, i);
            if lines[i + 1..end].iter().any(|l| re_def_get.is_match(l)) {
                descriptor_classes.insert(class_name);
            }
        }

        let mut descriptor_attrs: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for i in 0..lines.len() {
            let line = lines[i];
            let class_indent = indentation_width(line);
            let Some(_class_cap) = re_class.captures(line) else {
                continue;
            };
            let end = find_python_block_end(&lines, i);
            for l in &lines[i + 1..end] {
                if indentation_width(l) != class_indent + 4 {
                    continue;
                }
                let Some(cap) = re_class_attr_ctor.captures(l) else {
                    continue;
                };
                let Some(attr) = cap.get(1).map(|m| m.as_str().to_string()) else {
                    continue;
                };
                let Some(ctor) = cap.get(2).map(|m| m.as_str().to_string()) else {
                    continue;
                };
                if descriptor_classes.contains(&ctor) {
                    descriptor_attrs.insert(attr, ctor);
                }
            }
        }

        let mut descriptor_aliases: std::collections::HashMap<String, Vec<(u32, String)>> =
            std::collections::HashMap::new();
        if !descriptor_attrs.is_empty() {
            let re_alias_self_attr = Regex::new(
                r"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*self\.([A-Za-z_][A-Za-z0-9_]*)\s*$",
            )
            .expect("valid descriptor alias regex");
            for cap in re_alias_self_attr.captures_iter(source) {
                let Some(full) = cap.get(0) else {
                    continue;
                };
                let Some(alias) = cap.get(1).map(|m| m.as_str().to_string()) else {
                    continue;
                };
                let Some(attr) = cap.get(2).map(|m| m.as_str().to_string()) else {
                    continue;
                };
                let Some(desc_class) = descriptor_attrs.get(&attr).cloned() else {
                    continue;
                };
                let ln = byte_to_line(&offs, full.start());
                descriptor_aliases
                    .entry(alias)
                    .or_default()
                    .push((ln, desc_class));
            }
            for vals in descriptor_aliases.values_mut() {
                vals.sort_by_key(|(ln, _)| *ln);
            }

            for r in out.clone() {
                let direct_desc = if r.is_method && r.qualifier.as_deref() == Some("self") {
                    descriptor_attrs.get(&r.name).cloned()
                } else {
                    None
                };
                let alias_desc = if !r.is_method && r.qualifier.is_none() {
                    descriptor_aliases.get(&r.name).and_then(|vals| {
                        vals.iter()
                            .rev()
                            .find(|(ln, _)| *ln <= r.line)
                            .map(|(_, dc)| dc.clone())
                    })
                } else {
                    None
                };

                let Some(desc_class) = direct_desc.or(alias_desc) else {
                    continue;
                };

                let key = (
                    r.line,
                    "__get__".to_string(),
                    Some(desc_class.clone()),
                    true,
                );
                if !seen.insert(key) {
                    continue;
                }
                out.push(UnresolvedRef {
                    name: "__get__".to_string(),
                    kind: RefKind::Call,
                    file: path.to_string(),
                    line: r.line,
                    qualifier: Some(desc_class),
                    is_method: true,
                });
            }
        }

        // Conservative dynamic attribute fallback edges:
        // for unresolved self.<attr>(...) call sites inside classes that define
        // __getattr__ / __getattribute__, add inferred fallback refs to those dunder methods.
        #[derive(Clone)]
        struct ClassInfo {
            name: String,
            start_line: u32,
            end_line: u32,
            methods: std::collections::HashSet<String>,
            has_getattr: bool,
            has_getattribute: bool,
        }

        let re_method_def =
            Regex::new(r"^\s*def\s+([A-Za-z_][A-Za-z0-9_]*)\b").expect("valid python method regex");
        let mut class_infos: Vec<ClassInfo> = Vec::new();
        for i in 0..lines.len() {
            let Some(cap) = re_class.captures(lines[i]) else {
                continue;
            };
            let Some(class_name) = cap.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };

            let start_line = (i as u32) + 1;
            let end_idx = find_python_block_end(&lines, i);
            let end_line = end_idx as u32;
            let class_indent = indentation_width(lines[i]);
            let mut methods = std::collections::HashSet::new();

            for line in lines.iter().take(end_idx).skip(i + 1) {
                if indentation_width(line) <= class_indent {
                    continue;
                }
                if let Some(def_cap) = re_method_def.captures(line)
                    && let Some(method_name) = def_cap.get(1)
                {
                    methods.insert(method_name.as_str().to_string());
                }
            }

            if methods.is_empty() {
                continue;
            }

            let has_getattr = methods.contains("__getattr__");
            let has_getattribute = methods.contains("__getattribute__");
            if has_getattr || has_getattribute {
                class_infos.push(ClassInfo {
                    name: class_name,
                    start_line,
                    end_line,
                    methods,
                    has_getattr,
                    has_getattribute,
                });
            }
        }

        if !class_infos.is_empty() {
            for r in out.clone() {
                if !r.is_method || r.qualifier.as_deref() != Some("self") {
                    continue;
                }
                if r.name == "__getattr__" || r.name == "__getattribute__" {
                    continue;
                }

                let class_ctx = class_infos
                    .iter()
                    .filter(|c| r.line >= c.start_line && r.line <= c.end_line)
                    .max_by_key(|c| c.start_line);
                let Some(class_ctx) = class_ctx else {
                    continue;
                };
                if class_ctx.methods.contains(&r.name) {
                    continue;
                }

                if class_ctx.has_getattribute {
                    let key = (
                        r.line,
                        "__getattribute__".to_string(),
                        Some(class_ctx.name.clone()),
                        true,
                    );
                    if seen.insert(key) {
                        out.push(UnresolvedRef {
                            name: "__getattribute__".to_string(),
                            kind: RefKind::Call,
                            file: path.to_string(),
                            line: r.line,
                            qualifier: Some(class_ctx.name.clone()),
                            is_method: true,
                        });
                    }
                }

                if class_ctx.has_getattr {
                    let key = (
                        r.line,
                        "__getattr__".to_string(),
                        Some(class_ctx.name.clone()),
                        true,
                    );
                    if seen.insert(key) {
                        out.push(UnresolvedRef {
                            name: "__getattr__".to_string(),
                            kind: RefKind::Call,
                            file: path.to_string(),
                            line: r.line,
                            qualifier: Some(class_ctx.name.clone()),
                            is_method: true,
                        });
                    }
                }
            }
        }

        out
    }

    fn imports_in_file(
        &self,
        path: &str,
        source: &str,
    ) -> std::collections::HashMap<String, String> {
        use regex::Regex;

        let mut map = std::collections::HashMap::new();
        let from_mod = module_path_for_file(path);

        let re_import = Regex::new(r"(?m)^\s*import\s+(.+)$").unwrap();
        let re_from =
            Regex::new(r"(?m)^\s*from\s+([A-Za-z0-9_\.]+|\.+[A-Za-z0-9_\.]*)\s+import\s+(.+)$")
                .unwrap();

        for cap in re_import.captures_iter(source) {
            let rhs = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            for item in rhs.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                let (module_raw, alias) = if let Some((m, a)) = item.split_once(" as ") {
                    (m.trim(), a.trim())
                } else {
                    (item, item.split('.').next().unwrap_or(item).trim())
                };
                if module_raw.is_empty() || alias.is_empty() {
                    continue;
                }
                let module_path = module_raw.replace('.', "::");
                map.insert(alias.to_string(), module_path.clone());
                map.insert(format!("__glob__{}", module_path.clone()), module_path);
            }
        }

        for cap in re_from.captures_iter(source) {
            let module_raw = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let rhs = cap.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            if module_raw.is_empty() || rhs.is_empty() {
                continue;
            }

            let module_path = resolve_from_import_module(&from_mod, module_raw);
            if module_path.is_empty() {
                continue;
            }

            let rhs = rhs
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim_end_matches(',')
                .trim();

            for item in rhs.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                if item == "*" {
                    map.insert(
                        format!("__glob__{}", module_path.clone()),
                        module_path.clone(),
                    );
                    continue;
                }

                let (name, alias) = if let Some((n, a)) = item.split_once(" as ") {
                    (n.trim(), a.trim())
                } else {
                    (item, item)
                };
                if name.is_empty() || alias.is_empty() {
                    continue;
                }
                map.insert(alias.to_string(), format!("{}::{}", module_path, name));
            }
        }

        // Conservative dynamic import edges from importlib-style loaders.
        let offs = line_offsets(source);
        let re_call_importlib =
            Regex::new(r#"(?m)\bimportlib\.import_module\s*\(\s*([^,\)\n]+)"#).unwrap();
        let re_str_lit = Regex::new(r#"^[\"']([^\"']+)[\"']$"#).unwrap();
        let re_fstr_lit = Regex::new(r#"^f[\"']([^\"']+)[\"']$"#).unwrap();
        let re_ident = Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*)$"#).unwrap();
        let re_assign_expr = Regex::new(
            r#"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*|f?[\"'][^\n]+[\"'])\s*(?:#.*)?$"#,
        )
        .unwrap();

        let mut assigned_module_exprs: std::collections::HashMap<String, Vec<(u32, String)>> =
            std::collections::HashMap::new();
        for cap in re_assign_expr.captures_iter(source) {
            let Some(full) = cap.get(0) else {
                continue;
            };
            let Some(var) = cap.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };
            let Some(rhs) = cap.get(2).map(|m| m.as_str().trim().to_string()) else {
                continue;
            };
            let ln = byte_to_line(&offs, full.start());
            assigned_module_exprs
                .entry(var)
                .or_default()
                .push((ln, rhs));
        }
        for vals in assigned_module_exprs.values_mut() {
            vals.sort_by_key(|(ln, _)| *ln);
        }

        fn parse_dynamic_import_target(
            arg_raw: &str,
            line: u32,
            from_mod: &str,
            re_str_lit: &Regex,
            re_fstr_lit: &Regex,
            re_ident: &Regex,
            assigned_module_exprs: &std::collections::HashMap<String, Vec<(u32, String)>>,
            depth: u8,
        ) -> Option<String> {
            if depth == 0 {
                return None;
            }
            let raw = arg_raw.trim();
            let module_raw = if let Some(cap) = re_str_lit.captures(raw) {
                cap.get(1).map(|m| m.as_str().to_string())
            } else if let Some(cap) = re_fstr_lit.captures(raw) {
                let body = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let mut static_prefix = body.split('{').next().unwrap_or("").trim().to_string();
                while static_prefix.ends_with('.') {
                    static_prefix.pop();
                }
                if static_prefix.is_empty() {
                    None
                } else {
                    Some(static_prefix)
                }
            } else if let Some(cap) = re_ident.captures(raw) {
                let var = cap.get(1).map(|m| m.as_str())?;
                let rhs = assigned_module_exprs
                    .get(var)
                    .and_then(|vals| vals.iter().rev().find(|(ln, _)| *ln <= line))
                    .map(|(_, rhs)| rhs.clone())?;
                return parse_dynamic_import_target(
                    &rhs,
                    line,
                    from_mod,
                    re_str_lit,
                    re_fstr_lit,
                    re_ident,
                    assigned_module_exprs,
                    depth - 1,
                );
            } else {
                None
            }?;

            let resolved = if module_raw.starts_with('.') {
                resolve_from_import_module(from_mod, &module_raw)
            } else {
                module_raw.replace('.', "::")
            };
            if resolved.is_empty() {
                None
            } else {
                Some(resolved)
            }
        }

        let add_dynamic_import_edge =
            |map: &mut std::collections::HashMap<String, String>, ln: u32, arg: &str| {
                if let Some(module_path) = parse_dynamic_import_target(
                    arg,
                    ln,
                    &from_mod,
                    &re_str_lit,
                    &re_fstr_lit,
                    &re_ident,
                    &assigned_module_exprs,
                    4,
                ) {
                    map.insert(format!("__glob__{}", module_path.clone()), module_path);
                }
            };

        for cap in re_call_importlib.captures_iter(source) {
            let Some(full) = cap.get(0) else {
                continue;
            };
            let Some(arg) = cap.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let ln = byte_to_line(&offs, full.start());
            add_dynamic_import_edge(&mut map, ln, arg);
        }

        let import_module_fn_aliases: Vec<String> = map
            .iter()
            .filter_map(|(k, v)| (v == "importlib::import_module").then_some(k.clone()))
            .collect();
        for alias in import_module_fn_aliases {
            let pat = format!(r"(?m)\b{}\s*\(\s*([^,\)\n]+)", regex::escape(&alias));
            let re_call_alias = Regex::new(&pat).unwrap();
            for cap in re_call_alias.captures_iter(source) {
                let Some(full) = cap.get(0) else {
                    continue;
                };
                let Some(arg) = cap.get(1).map(|m| m.as_str()) else {
                    continue;
                };
                let ln = byte_to_line(&offs, full.start());
                add_dynamic_import_edge(&mut map, ln, arg);
            }
        }

        let importlib_module_aliases: Vec<String> = map
            .iter()
            .filter_map(|(k, v)| (v == "importlib").then_some(k.clone()))
            .collect();
        for alias in importlib_module_aliases {
            let pat = format!(
                r"(?m)\b{}\.import_module\s*\(\s*([^,\)\n]+)",
                regex::escape(&alias)
            );
            let re_call_alias = Regex::new(&pat).unwrap();
            for cap in re_call_alias.captures_iter(source) {
                let Some(full) = cap.get(0) else {
                    continue;
                };
                let Some(arg) = cap.get(1).map(|m| m.as_str()) else {
                    continue;
                };
                let ln = byte_to_line(&offs, full.start());
                add_dynamic_import_edge(&mut map, ln, arg);
            }
        }

        let re_call_dunder_import = Regex::new(r#"(?m)\b__import__\s*\(\s*([^,\)\n]+)"#).unwrap();
        for cap in re_call_dunder_import.captures_iter(source) {
            let Some(full) = cap.get(0) else {
                continue;
            };
            let Some(arg) = cap.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let ln = byte_to_line(&offs, full.start());
            add_dynamic_import_edge(&mut map, ln, arg);
        }

        let dunder_aliases: Vec<String> = map
            .iter()
            .filter_map(|(k, v)| {
                (v == "builtins::__import__" || v == "__import__").then_some(k.clone())
            })
            .collect();
        for alias in dunder_aliases {
            let pat = format!(r"(?m)\b{}\s*\(\s*([^,\)\n]+)", regex::escape(&alias));
            let re_call_alias = Regex::new(&pat).unwrap();
            for cap in re_call_alias.captures_iter(source) {
                let Some(full) = cap.get(0) else {
                    continue;
                };
                let Some(arg) = cap.get(1).map(|m| m.as_str()) else {
                    continue;
                };
                let ln = byte_to_line(&offs, full.start());
                add_dynamic_import_edge(&mut map, ln, arg);
            }
        }

        map
    }
}

fn indentation_width(s: &str) -> usize {
    let mut w = 0usize;
    for ch in s.chars() {
        match ch {
            ' ' => w += 1,
            '\t' => w += 4,
            _ => break,
        }
    }
    w
}

fn find_python_block_end(lines: &[&str], start_idx: usize) -> usize {
    let base = indentation_width(lines[start_idx]);
    for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if indentation_width(line) <= base {
            return i;
        }
    }
    lines.len()
}

fn module_path_for_file(path: &str) -> String {
    let p = std::path::Path::new(path);
    let mut s = p.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    if s.ends_with("/__init__.py") {
        s = s.trim_end_matches("/__init__.py").to_string();
    } else if s.ends_with(".py") {
        s = s.trim_end_matches(".py").to_string();
    }
    s.replace('/', "::")
}

fn resolve_from_import_module(from_mod: &str, module_raw: &str) -> String {
    let dots = module_raw.chars().take_while(|c| *c == '.').count();
    if dots == 0 {
        return module_raw.replace('.', "::");
    }

    let rest = module_raw[dots..].trim_matches('.').replace('.', "::");
    let mut parts: Vec<&str> = from_mod.split("::").filter(|s| !s.is_empty()).collect();
    // current file module -> package scope
    if !parts.is_empty() {
        parts.pop();
    }
    for _ in 1..dots {
        if !parts.is_empty() {
            parts.pop();
        }
    }

    let mut base = parts.join("::");
    if !rest.is_empty() {
        if !base.is_empty() {
            base.push_str("::");
        }
        base.push_str(&rest);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageAnalyzer;

    #[test]
    fn extract_python_symbols_functions_classes_and_methods() {
        let src = r#"class Service:
    def handle(self, v):
        return v

def run(x):
    return x
"#;
        let ana = SpecPyAnalyzer::new();
        let syms = ana.symbols_in_file("main.py", src);

        assert!(syms.iter().any(|s| {
            s.name == "Service"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:main.py:struct:Service:1"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "handle"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:main.py:method:handle:2"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Function)
                && s.id.0 == "python:main.py:fn:run:5"
        }));
    }

    #[test]
    fn unresolved_refs_extracts_bare_and_qualified_calls() {
        let src = r#"def call_all(obj):
    foo()
    obj.bar()
    self.baz()
"#;
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("main.py", src);

        assert!(
            refs.iter().any(|r| {
                r.name == "foo" && r.qualifier.is_none() && !r.is_method && r.line == 2
            })
        );
        assert!(refs.iter().any(|r| {
            r.name == "bar" && r.qualifier.as_deref() == Some("obj") && r.is_method && r.line == 3
        }));
        assert!(refs.iter().any(|r| {
            r.name == "baz" && r.qualifier.as_deref() == Some("self") && r.is_method && r.line == 4
        }));
    }

    #[test]
    fn unresolved_refs_marks_import_alias_calls_as_non_method() {
        let src = r#"import importlib
from pkg import mod as local_mod

def run():
    importlib.import_module('x')
    local_mod.run()
"#;
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("pkg/main.py", src);

        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("importlib") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "run" && r.qualifier.as_deref() == Some("local_mod") && !r.is_method
        }));
    }

    #[test]
    fn unresolved_refs_extracts_decorator_identifier_and_qualified_decorator() {
        let src = r#"@traced
@pkg.decorate
@pkg.wrap(1)
def run():
    return 1
"#;
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("pkg/main.py", src);

        assert!(refs.iter().any(|r| {
            r.name == "traced" && r.line == 4 && r.qualifier.is_none() && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "decorate"
                && r.line == 4
                && r.qualifier.as_deref() == Some("pkg")
                && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "wrap" && r.line == 4 && r.qualifier.as_deref() == Some("pkg") && !r.is_method
        }));
    }

    #[test]
    fn imports_extract_alias_from_and_relative_paths() {
        let src = r#"import os
import util.helpers as uh
from pkg.service import run as runner, Client
from .local import fn as local_fn
from ..core import base
from . import sibling
from pkg.star import *
"#;
        let ana = SpecPyAnalyzer::new();
        let im = ana.imports_in_file("pkg/sub/main.py", src);

        assert_eq!(im.get("os").map(String::as_str), Some("os"));
        assert_eq!(im.get("uh").map(String::as_str), Some("util::helpers"));
        assert_eq!(
            im.get("runner").map(String::as_str),
            Some("pkg::service::run")
        );
        assert_eq!(
            im.get("Client").map(String::as_str),
            Some("pkg::service::Client")
        );
        assert_eq!(
            im.get("local_fn").map(String::as_str),
            Some("pkg::sub::local::fn")
        );
        assert_eq!(im.get("base").map(String::as_str), Some("pkg::core::base"));
        assert_eq!(
            im.get("sibling").map(String::as_str),
            Some("pkg::sub::sibling")
        );
        assert_eq!(
            im.get("__glob__pkg::star").map(String::as_str),
            Some("pkg::star")
        );
    }

    #[test]
    fn resolve_from_import_module_handles_relative_levels() {
        assert_eq!(
            resolve_from_import_module("pkg::sub::main", ".local"),
            "pkg::sub::local"
        );
        assert_eq!(
            resolve_from_import_module("pkg::sub::main", "..core"),
            "pkg::core"
        );
        assert_eq!(
            resolve_from_import_module("pkg::sub::main", "pkg.service"),
            "pkg::service"
        );
    }

    #[test]
    fn python_hard_case_fixture_dynamic_call_and_import_edge() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/sub/main.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "DynamicRunner"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/sub/main.py:struct:DynamicRunner:7"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/sub/main.py:method:run:8"
        }));

        let refs = ana.unresolved_refs("pkg/sub/main.py", src);
        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("importlib") && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "imod" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "load" && r.qualifier.as_deref() == Some("local_loader") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "handle" && r.qualifier.as_deref() == Some("handler") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "process" && r.qualifier.as_deref() == Some("extra") && r.is_method
        }));

        let im = ana.imports_in_file("pkg/sub/main.py", src);
        assert_eq!(im.get("importlib").map(String::as_str), Some("importlib"));
        assert_eq!(
            im.get("imod").map(String::as_str),
            Some("importlib::import_module")
        );
        assert_eq!(
            im.get("local_loader").map(String::as_str),
            Some("pkg::sub::plugins::loader")
        );
        assert_eq!(
            im.get("__glob__pkg::sub::plugins").map(String::as_str),
            Some("pkg::sub::plugins")
        );
    }

    #[test]
    fn python_hard_case_fixture_v041_dynamic_call_import_edge() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_v041.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/sub/main.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "DynamicRunner"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/sub/main.py:struct:DynamicRunner:7"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/sub/main.py:method:run:8"
        }));

        let refs = ana.unresolved_refs("pkg/sub/main.py", src);
        assert!(
            refs.iter()
                .any(|r| r.name == "imod" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("il") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "load" && r.qualifier.as_deref() == Some("loader_mod") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "handle" && r.qualifier.as_deref() == Some("handler") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "process" && r.qualifier.as_deref() == Some("via_alias") && r.is_method
        }));

        let im = ana.imports_in_file("pkg/sub/main.py", src);
        assert_eq!(im.get("il").map(String::as_str), Some("importlib"));
        assert_eq!(
            im.get("imod").map(String::as_str),
            Some("importlib::import_module")
        );
        assert_eq!(
            im.get("loader_mod").map(String::as_str),
            Some("pkg::sub::plugins::loader")
        );
        assert_eq!(
            im.get("__glob__pkg::sub::plugins").map(String::as_str),
            Some("pkg::sub::plugins")
        );
        assert_eq!(
            im.get("__glob__plugins::extra").map(String::as_str),
            Some("plugins::extra")
        );
    }

    #[test]
    fn imports_in_file_conservative_dynamic_import_handles_aliases_and_dunder() {
        let src = r#"import importlib as il
from builtins import __import__ as dyn_import


def run(name):
    target = ".plugins.runtime"
    mod_a = il.import_module(target)

    common_mod = "pkg.plugins.common"
    mod_b = dyn_import(common_mod)

    dynamic_target = f".ext.{name}"
    mod_c = __import__(dynamic_target)
    return mod_a, mod_b, mod_c
"#;
        let ana = SpecPyAnalyzer::new();
        let im = ana.imports_in_file("pkg/sub/main.py", src);

        assert_eq!(im.get("il").map(String::as_str), Some("importlib"));
        assert_eq!(
            im.get("dyn_import").map(String::as_str),
            Some("builtins::__import__")
        );
        assert_eq!(
            im.get("__glob__pkg::sub::plugins::runtime")
                .map(String::as_str),
            Some("pkg::sub::plugins::runtime")
        );
        assert_eq!(
            im.get("__glob__pkg::plugins::common").map(String::as_str),
            Some("pkg::plugins::common")
        );
        assert_eq!(
            im.get("__glob__pkg::sub::ext").map(String::as_str),
            Some("pkg::sub::ext")
        );
    }

    #[test]
    fn unresolved_refs_descriptor_alias_call_infers_get_edge() {
        let src = r#"class UpperDescriptor:
    def __get__(self, obj, objtype=None):
        def inner(v):
            return v.upper()

        return inner


class Service:
    normalizer = UpperDescriptor()

    def process(self, value):
        fn = self.normalizer
        return fn(value)
"#;
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("pkg/svc_alias.py", src);
        let refs_dbg: Vec<_> = refs
            .iter()
            .map(|r| match &r.qualifier {
                Some(q) => format!("{}@{}[{}]/{}", r.name, r.line, q, r.is_method),
                None => format!("{}@{}/{}", r.name, r.line, r.is_method),
            })
            .collect();

        assert!(
            refs.iter().any(|r| {
                r.name == "__get__"
                    && r.qualifier.as_deref() == Some("UpperDescriptor")
                    && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
    }

    #[test]
    fn python_hard_case_fixture_decorator_descriptor_attribute_chain() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_decorator_descriptor_chain.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/svc.py", src);
        assert!(
            syms.iter()
                .any(|s| s.name == "UpperDescriptor" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "Service" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "process" && matches!(s.kind, SymbolKind::Method))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "traced" && matches!(s.kind, SymbolKind::Function))
        );

        let refs = ana.unresolved_refs("pkg/svc.py", src);
        assert!(
            refs.iter()
                .any(|r| r.name == "w" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| r.name == "traced" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "normalizer" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "send"
                && r.qualifier.as_deref() == Some("self.api.client.dispatcher")
                && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "strip" && r.qualifier.as_deref() == Some("cleaned") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "lower"
                && r.qualifier
                    .as_deref()
                    .map(|q| q.starts_with("cleaned.strip"))
                    .unwrap_or(false)
                && r.is_method
        }));

        let im = ana.imports_in_file("pkg/svc.py", src);
        assert_eq!(im.get("w").map(String::as_str), Some("functools::wraps"));
    }

    #[test]
    fn python_hard_case_fixture_dynamic_getattr_setattr_getattr_dunder() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_dynamic_getattr_setattr_getattribute.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/dynamic.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "DynamicAccessor"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/dynamic.py:struct:DynamicAccessor:1"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "__getattr__"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/dynamic.py:method:__getattr__:5"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "execute"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/dynamic.py:method:execute:10"
        }));

        let refs = ana.unresolved_refs("pkg/dynamic.py", src);
        assert!(
            refs.iter()
                .any(|r| r.name == "setattr" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| r.name == "getattr" && r.qualifier.is_none() && !r.is_method)
        );
        // getattr/setattr dynamic target edge (conservative)
        assert!(refs.iter().any(|r| {
            r.name == "bound_handler" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "dyn_method" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "dyn_value" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "__getattr__"
                && r.line == 13
                && r.qualifier.as_deref() == Some("DynamicAccessor")
                && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "strip" && r.qualifier.as_deref() == Some("payload") && r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "lower" && r.qualifier.as_deref() == Some("v") && r.is_method)
        );
    }

    #[test]
    fn unresolved_refs_adds_getattribute_getattr_fallback_edges_for_dynamic_self_calls() {
        let src = r#"class DynamicAttr:
    def __getattribute__(self, name):
        return super().__getattribute__(name)

    def __getattr__(self, name):
        if name == "dyn_call":
            return lambda payload: payload.strip()
        raise AttributeError(name)

    def run(self, payload):
        setter_name = "dyn_field"
        setattr(self, setter_name, payload)
        getattr(self, "dyn_call")(payload)
        self.unknown_method(payload)
"#;

        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("pkg/dynamic_attr.py", src);

        assert!(refs.iter().any(|r| {
            r.name == "dyn_field" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "dyn_call" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "__getattribute__"
                && r.line == 13
                && r.qualifier.as_deref() == Some("DynamicAttr")
                && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "__getattr__"
                && r.line == 13
                && r.qualifier.as_deref() == Some("DynamicAttr")
                && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "__getattribute__"
                && r.line == 14
                && r.qualifier.as_deref() == Some("DynamicAttr")
                && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "__getattr__"
                && r.line == 14
                && r.qualifier.as_deref() == Some("DynamicAttr")
                && r.is_method
        }));
    }

    #[test]
    fn python_hard_case_fixture_dynamic_descriptor_decorator_chain() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_dynamic_descriptor_decorator_chain.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/dynamic_chain.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "TrimDescriptor"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/dynamic_chain.py:struct:TrimDescriptor:4"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "Pipeline"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/dynamic_chain.py:struct:Pipeline:31"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/dynamic_chain.py:method:run:39"
        }));

        let refs = ana.unresolved_refs("pkg/dynamic_chain.py", src);
        assert!(
            refs.iter()
                .any(|r| r.name == "wraps" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| r.name == "audited" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| r.name == "traced" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "trim" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "__get__" && r.qualifier.as_deref() == Some("TrimDescriptor") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "emit" && r.qualifier.as_deref() == Some("self.sink.client") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "lower" && r.qualifier.as_deref() == Some("cleaned") && r.is_method
        }));

        let im = ana.imports_in_file("pkg/dynamic_chain.py", src);
        assert_eq!(
            im.get("wraps").map(String::as_str),
            Some("functools::wraps")
        );
    }

    #[test]
    fn python_hard_case_fixture_dynamic_importlib_edge() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_dynamic_importlib_edge.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/sub/main.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "DynamicImportEdge"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/sub/main.py:struct:DynamicImportEdge:8"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/sub/main.py:method:run:9"
        }));

        let refs = ana.unresolved_refs("pkg/sub/main.py", src);
        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("importlib") && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "imod" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| r.name == "getattr" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "build" && r.qualifier.as_deref() == Some("mod1") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "make" && r.qualifier.as_deref() == Some("mod2") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "load" && r.qualifier.as_deref() == Some("loader_mod") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "lookup" && r.qualifier.as_deref() == Some("registry") && !r.is_method
        }));

        let im = ana.imports_in_file("pkg/sub/main.py", src);
        assert_eq!(im.get("importlib").map(String::as_str), Some("importlib"));
        assert_eq!(
            im.get("imod").map(String::as_str),
            Some("importlib::import_module")
        );
        assert_eq!(
            im.get("loader_mod").map(String::as_str),
            Some("pkg::sub::plugins::loader")
        );
        assert_eq!(
            im.get("registry").map(String::as_str),
            Some("pkg::core::registry")
        );
        // conservative dynamic import edges
        assert_eq!(
            im.get("__glob__pkg::sub::plugins").map(String::as_str),
            Some("pkg::sub::plugins")
        );
        assert_eq!(
            im.get("__glob__pkg::plugins::common").map(String::as_str),
            Some("pkg::plugins::common")
        );
    }
}
