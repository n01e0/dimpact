use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Spec {
    pub language: String,
    pub queries: Queries,
}

#[derive(Debug, Deserialize)]
pub struct Queries {
    pub declarations: String,
    pub calls: String,
    pub imports: String,
    #[serde(default)]
    pub control: String,
}

pub fn load_rust_spec() -> Spec {
    static YAML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/specs/rust.yml"));
    serde_yaml::from_str(YAML).expect("valid rust spec yaml")
}

pub fn load_ruby_spec() -> Spec {
    static YAML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/specs/ruby.yml"));
    serde_yaml::from_str(YAML).expect("valid ruby spec yaml")
}


pub struct CompiledQueries {
    pub decl: tree_sitter::Query,
    pub calls: tree_sitter::Query,
    pub imports: tree_sitter::Query,
    pub control: Option<tree_sitter::Query>,
}

pub fn compile_queries_rust(spec: &Spec) -> anyhow::Result<CompiledQueries> {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let decl = tree_sitter::Query::new(&lang, &spec.queries.declarations)?;
    let calls = tree_sitter::Query::new(&lang, &spec.queries.calls)?;
    let imports = tree_sitter::Query::new(&lang, &spec.queries.imports)?;
    let control = if spec.queries.control.trim().is_empty() { None } else { Some(tree_sitter::Query::new(&lang, &spec.queries.control)?) };
    Ok(CompiledQueries { decl, calls, imports, control })
}

pub fn compile_queries_ruby(spec: &Spec) -> anyhow::Result<CompiledQueries> {
    let lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
    let decl = tree_sitter::Query::new(&lang, &spec.queries.declarations)?;
    let calls = tree_sitter::Query::new(&lang, &spec.queries.calls)?;
    let imports = tree_sitter::Query::new(&lang, &spec.queries.imports)?;
    let control = if spec.queries.control.trim().is_empty() { None } else { Some(tree_sitter::Query::new(&lang, &spec.queries.control)?) };
    Ok(CompiledQueries { decl, calls, imports, control })
}

pub fn load_javascript_spec() -> Spec {
    static YAML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/specs/javascript.yml"));
    serde_yaml::from_str(YAML).expect("valid javascript spec yaml")
}

pub fn compile_queries_javascript(spec: &Spec) -> anyhow::Result<CompiledQueries> {
    let lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
    let decl = tree_sitter::Query::new(&lang, &spec.queries.declarations)?;
    let calls = tree_sitter::Query::new(&lang, &spec.queries.calls)?;
    let imports = tree_sitter::Query::new(&lang, &spec.queries.imports)?;
    let control = if spec.queries.control.trim().is_empty() { None } else { Some(tree_sitter::Query::new(&lang, &spec.queries.control)?) };
    Ok(CompiledQueries { decl, calls, imports, control })
}

pub fn load_typescript_spec() -> Spec {
    static YAML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/specs/typescript.yml"));
    serde_yaml::from_str(YAML).expect("valid typescript spec yaml")
}

pub fn compile_queries_typescript(spec: &Spec, tsx: bool) -> anyhow::Result<CompiledQueries> {
    let lang: tree_sitter::Language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let decl = tree_sitter::Query::new(&lang, &spec.queries.declarations)?;
    let calls = tree_sitter::Query::new(&lang, &spec.queries.calls)?;
    let imports = tree_sitter::Query::new(&lang, &spec.queries.imports)?;
    let control = if spec.queries.control.trim().is_empty() { None } else { Some(tree_sitter::Query::new(&lang, &spec.queries.control)?) };
    Ok(CompiledQueries { decl, calls, imports, control })
}


pub struct QueryRunner {
    parser: std::cell::RefCell<tree_sitter::Parser>,
}

impl QueryRunner {
    pub fn new_rust() -> Self {
        let mut p = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        p.set_language(&lang).expect("lang");
        Self { parser: std::cell::RefCell::new(p) }
    }

    pub fn new_ruby() -> Self {
        let mut p = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
        p.set_language(&lang).expect("lang");
        Self { parser: std::cell::RefCell::new(p) }
    }

    pub fn new_javascript() -> Self {
        let mut p = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
        p.set_language(&lang).expect("lang");
        Self { parser: std::cell::RefCell::new(p) }
    }

    pub fn new_typescript(tsx: bool) -> Self {
        let mut p = tree_sitter::Parser::new();
        let lang: tree_sitter::Language = if tsx { tree_sitter_typescript::LANGUAGE_TSX.into() } else { tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into() };
        p.set_language(&lang).expect("lang");
        Self { parser: std::cell::RefCell::new(p) }
    }


    pub fn run_captures(&self, src: &str, q: &tree_sitter::Query) -> Vec<Vec<Capture>> {
        let tree = self.parser.borrow_mut().parse(src, None).expect("parse");
        let root = tree.root_node();
        let mut qc = tree_sitter::QueryCursor::new();
        let names = q.capture_names();
        let mut out = Vec::new();
        for m in qc.matches(q, root, src.as_bytes()) {
            let mut caps = Vec::with_capacity(m.captures.len());
            for c in m.captures {
                let name = names[c.index as usize].to_string();
                let start = c.node.start_byte();
                let end = c.node.end_byte();
                let kind = c.node.kind().to_string();
                caps.push(Capture { name, start, end, kind });
            }
            out.push(caps);
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_spec_loads_and_matches() {
        let spec = load_rust_spec();
        let compiled = compile_queries_rust(&spec).unwrap();
        let src = r#"use crate::m::n::{self, x};
struct S; impl S { fn m(&self) {} }
fn foo() { bar(); S.m; let s=S; s.m(); crate::m::x(); }
fn bar() {}
"#;
        let qr = QueryRunner::new_rust();
        let decls = qr.run_captures(src, &compiled.decl);
        assert!(!decls.is_empty(), "should match decls");
        let calls = qr.run_captures(src, &compiled.calls);
        assert!(!calls.is_empty(), "should match calls");
        let uses = qr.run_captures(src, &compiled.imports);
        assert_eq!(uses.len(), 1, "one use declaration");
    }
}
