class AliasDefineDynamic
  def original
    :ok
  end

  alias_method :aliased_sym, :original
  alias_method "aliased_str", :original

  define_method(:defined_sym) do
    aliased_sym
  end

  define_method("defined_str") do
    aliased_str
  end

  def execute
    defined_sym
    defined_str
  end
end
