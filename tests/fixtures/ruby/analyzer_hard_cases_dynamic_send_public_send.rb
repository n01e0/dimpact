class DynamicDispatch
  def target_sym
    :ok
  end

  def target_str
    :ok
  end

  def execute
    send(:target_sym)
    public_send(:target_sym)

    send("target_str")
    public_send("target_str")

    dyn_sym = :target_sym
    send(dyn_sym)

    dyn_str = "target_str"
    public_send(dyn_str)
  end
end
