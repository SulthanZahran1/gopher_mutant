package large

import "testing"

func TestMath(t *testing.T) {
	if Add(2, 3) != 5 || Multiply(3, 4) != 12 || Difference(8, 3) != 5 || Quotient(8, 2) != 4 {
		t.Fatal("math mismatch")
	}
	if !MathWindow(0) || !MathWindow(100) || MathWindow(101) {
		t.Fatal("window mismatch")
	}
	if MathBare(3) != 4 {
		t.Fatal("bare math is asserted")
	}
}

func TestPredicates(t *testing.T) {
	if !InWindow(15) || InWindow(25) {
		t.Fatal("window predicate mismatch")
	}
	if !IsEven(4) || IsEven(3) {
		t.Fatal("even predicate mismatch")
	}
	if PredicatePair(2) != true || PredicatePair(8) != true || PredicatePair(1) {
		t.Fatal("pair predicate mismatch")
	}
	PredicateBare(1)
	PredicatePairBare(10)
}

func TestLoops(t *testing.T) {
	if LoopTimeout(2) != 1 {
		t.Fatal("timeout loop mismatch")
	}
	if LoopSafe(6) != 6 {
		t.Fatal("safe loop mismatch")
	}
	LoopSafeBare(6)
	LoopConditionBare(1)
}

func TestRecords(t *testing.T) {
	record := Record{value: 4, label: "ok"}
	var value Valuer = record
	if ReadValue(value) != 5 || record.Value() != 5 || !record.Enabled() {
		t.Fatal("record mismatch")
	}
	if RecordExact(record) != 8 {
		t.Fatal("record exact mismatch")
	}
	RecordBare(record)
}

func TestErrors(t *testing.T) {
	if value, err := Check(4); err != nil || value != 4 {
		t.Fatal("check mismatch")
	}
	if _, err := Check(-1); err == nil {
		t.Fatal("negative check mismatch")
	}
	if value, label := Pair(4); value != 4 || label != "ok" {
		t.Fatal("pair mismatch")
	}
	if DeferResult() != 2 {
		t.Fatal("defer mismatch")
	}
	if RecoverResult() != 3 {
		t.Fatal("recover mismatch")
	}
	ErrorBare(3)
}

func TestChannels(t *testing.T) {
	if AsyncValue() != 8 || GoroutineBare() != 1 || CloseValue() != 9 {
		t.Fatal("channel mismatch")
	}
	ready := make(chan int, 1)
	ready <- 7
	if SelectReady(ready) != 7 {
		t.Fatal("select mismatch")
	}
	other := make(chan int, 1)
	other <- 9
	SelectBare(other)
}

func TestCollections(t *testing.T) {
	if MapTotal(map[string]int{"a": 2, "b": 3}) != 5 {
		t.Fatal("map mismatch")
	}
	if RangeTotal([]int{1, 2, 3}) != 6 {
		t.Fatal("range mismatch")
	}
	RangeBare([]int{1, 2})
	if got := AppendList([]int{1, 2}); len(got) != 3 || got[2] != 4 {
		t.Fatal("append mismatch")
	}
	if SlicePick([]int{3, 4, 5}, 0) != 3 {
		t.Fatal("slice mismatch")
	}
}
