fn leaf() {
    let base = 1;
    let _ = base + 1;
}

fn first_wave() {
    leaf();
}

fn second_wave() {
    first_wave();
}

fn third_wave() {
    second_wave();
}

fn entry() {
    third_wave();
}
