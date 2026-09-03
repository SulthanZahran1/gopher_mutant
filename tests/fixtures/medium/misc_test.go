package medium

import "testing"

func TestArithmetic(t *testing.T) {
	if Add(2, 3) != 5 || Multiply(3, 4) != 12 {
		t.Fatal("arithmetic mismatch")
	}
	if !InRange(5, 0, 10) || !InRange(0, 0, 10) || !InRange(10, 0, 10) {
		t.Fatal("range mismatch")
	}
	if InRange(-1, 0, 10) || InRange(11, 0, 10) {
		t.Fatal("range boundary mismatch")
	}
	if LoopTotal(4) != 6 {
		t.Fatal("loop mismatch")
	}
	if Outside(0, 0, 10) != 0 || Outside(10, 0, 10) != 0 || Outside(11, 0, 10) != 1 || Outside(-1, 0, 10) != 1 {
		t.Fatal("outside mismatch")
	}
	if value, err := CheckedValue(4); err != nil || value != 4 {
		t.Fatal("checked value mismatch")
	}
	if _, err := CheckedValue(-1); err == nil {
		t.Fatal("negative value was accepted")
	}
	if value, err := CheckedError(4); err != nil || value != 4 {
		t.Fatal("checked error value mismatch")
	}
	if _, err := CheckedError(-1); err == nil {
		t.Fatal("checked error was ignored")
	}
	if value, label := ReturnPair(4); value != 4 || label != "ok" {
		t.Fatal("pair mismatch")
	}
}

func TestIdioms(t *testing.T) {
	if DeferredValue() != 7 {
		t.Fatal("defer mismatch")
	}
	if AsyncValue() != 8 {
		t.Fatal("goroutine mismatch")
	}
	if ClosedChannelValue() != 9 {
		t.Fatal("close mismatch")
	}
	ready := make(chan int, 1)
	ready <- 5
	if SelectValue(ready) != 5 {
		t.Fatal("select mismatch")
	}
	if got := AppendValue([]int{1, 2}); len(got) != 3 || got[2] != 4 {
		t.Fatal("append mismatch")
	}
	if SliceValue([]int{3, 4, 5}, 0) != 3 {
		t.Fatal("slice mismatch")
	}
	if RecoverValue() != 3 {
		t.Fatal("recover mismatch")
	}
}

func TestInterfaces(t *testing.T) {
	box := Box{value: 11}
	if UseBox(box) != 11 {
		t.Fatal("method mismatch")
	}
	if MapTotal(map[string]int{"a": 2, "b": 3}) != 5 {
		t.Fatal("map mismatch")
	}
	if RangeTotal([]int{1, 2, 3}) != 6 {
		t.Fatal("range mismatch")
	}
	MapSurvivor(map[string]int{"x": 1})
	UncheckedArithmetic(4)
	UncheckedRelation(1)
	TestableCalls(3)
	NotCoveredChange(3)
	NotCoveredFlag(3)
	if !NotCoveredFlag(6) || NotCoveredFlag(7) {
		t.Fatal("flag boundary mismatch")
	}
}
