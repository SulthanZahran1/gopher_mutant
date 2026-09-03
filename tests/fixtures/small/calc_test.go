package calc

import "testing"

func TestAdd(t *testing.T) {
	if Add(1, 2) != 3 || Add(0, 0) != 0 {
		t.Fatal("Add returned the wrong value")
	}
}

func TestMul(t *testing.T) {
	if Mul(3, 4) != 12 || Mul(0, 7) != 0 {
		t.Fatal("Mul returned the wrong value")
	}
}

func TestIsPositive(t *testing.T) {
	if !IsPositive(5) || IsPositive(0) || IsPositive(-1) {
		t.Fatal("IsPositive returned the wrong value")
	}
}

func TestIsInRange(t *testing.T) {
	for _, test := range []struct {
		value int
		want  bool
	}{
		{value: 5, want: true},
		{value: 0, want: true},
		{value: 10, want: true},
		{value: -1, want: false},
		{value: 15, want: false},
	} {
		if got := IsInRange(test.value, 0, 10); got != test.want {
			t.Fatalf("IsInRange(%d) = %v, want %v", test.value, got, test.want)
		}
	}
}

func TestSumEven(t *testing.T) {
	if SumEven(6) != 6 || SumEven(0) != 0 || SumEven(1) != 0 {
		t.Fatal("SumEven returned the wrong value")
	}
}

func TestCountClamped(t *testing.T) {
	cases := []struct {
		value int
		want  int
	}{
		{value: 5, want: 0},
		{value: 0, want: 0},
		{value: 10, want: 0},
		{value: 15, want: 1},
		{value: -1, want: 1},
	}
	for _, test := range cases {
		if got := CountClamped(test.value, 0, 10); got != test.want {
			t.Fatalf("CountClamped(%d) = %d, want %d", test.value, got, test.want)
		}
	}
}

func TestOffset(t *testing.T) {
	if Offset(3) != 6 {
		t.Fatal("Offset returned the wrong value")
	}
}

func TestDeferredValue(t *testing.T) {
	if DeferredValue() != 1 {
		t.Fatal("DeferredValue returned the wrong value")
	}
}
