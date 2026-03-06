package main

func f00() int {
	v := 0
	return v
}

func f01() int {
	v := f00()
	inc := 1
	return v + inc
}

func f02() int {
	v := f01()
	inc := 1
	return v + inc
}

func f03() int {
	v := f02()
	inc := 1
	return v + inc
}

func f04() int {
	v := f03()
	inc := 1
	return v + inc
}

func f05() int {
	v := f04()
	inc := 1
	return v + inc
}

func f06() int {
	v := f05()
	inc := 1
	return v + inc
}

func f07() int {
	v := f06()
	inc := 1
	return v + inc
}

func f08() int {
	v := f07()
	inc := 1
	return v + inc
}

func f09() int {
	v := f08()
	inc := 1
	return v + inc
}

func f10() int {
	return f09() + 1
}

func f11() int {
	return f10() + 1
}

func f12() int {
	return f11() + 1
}

func f13() int {
	return f12() + 1
}

func f14() int {
	return f13() + 1
}

func f15() int {
	return f14() + 1
}

func f16() int {
	return f15() + 1
}

func f17() int {
	return f16() + 1
}

func f18() int {
	return f17() + 1
}

func f19() int {
	return f18() + 1
}

func f20() int {
	return f19() + 1
}

func f21() int {
	return f20() + 1
}

func f22() int {
	return f21() + 1
}

func f23() int {
	return f22() + 1
}

func f24() int {
	return f23() + 1
}

func f25() int {
	return f24() + 1
}

func f26() int {
	return f25() + 1
}

func f27() int {
	return f26() + 1
}

func f28() int {
	return f27() + 1
}

func f29() int {
	return f28() + 1
}

func f30() int {
	return f29() + 1
}

func f31() int {
	return f30() + 1
}

func f32() int {
	return f31() + 1
}

func f33() int {
	return f32() + 1
}

func f34() int {
	return f33() + 1
}

func f35() int {
	return f34() + 1
}

func f36() int {
	return f35() + 1
}

func f37() int {
	return f36() + 1
}

func f38() int {
	return f37() + 1
}

func f39() int {
	return f38() + 1
}

func main() {
	_ = f39()
}
