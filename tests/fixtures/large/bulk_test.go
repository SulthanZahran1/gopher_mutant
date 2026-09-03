package large

import "testing"

func TestBulkExact(t *testing.T) {
	if BulkA0(3) != 6 || BulkA2(3) != 6 || BulkA4(3) != 6 || BulkA6(3) != 6 || BulkA8(3) != 6 || BulkA10(3) != 6 {
		t.Fatal("bulk arithmetic mismatch")
	}
	for _, test := range []struct {
		fn   func(int) bool
		name string
	}{
		{BulkB0, "B0"},
		{BulkB2, "B2"},
		{BulkB4, "B4"},
		{BulkB6, "B6"},
		{BulkB8, "B8"},
		{BulkB10, "B10"},
	} {
		for _, value := range []int{0, 1, 9, 10} {
			want := value >= 1 && value <= 9
			if got := test.fn(value); got != want {
				t.Fatalf("%s(%d) = %v, want %v", test.name, value, got, want)
			}
		}
	}
	if BulkC0(6) != 6 || BulkC2(6) != 6 {
		t.Fatal("bulk loop mismatch")
	}
}

func TestBulkSurvivors(t *testing.T) {
	if BulkA1(2) != 5 || BulkA1(3) != 6 || BulkA3(2) != 5 || BulkA3(3) != 6 {
		t.Fatal("bulk arithmetic boundary mismatch")
	}
	for _, test := range []struct {
		fn   func(int) bool
		name string
	}{
		{BulkB1, "B1"},
		{BulkB3, "B3"},
	} {
		for _, value := range []int{0, 1, 9, 10} {
			want := value >= 1 && value <= 9
			if got := test.fn(value); got != want {
				t.Fatalf("%s(%d) = %v, want %v", test.name, value, got, want)
			}
		}
	}
	BulkA5(3)
	BulkA7(3)
	BulkA9(3)
	BulkA11(3)
	BulkB5(3)
	BulkB7(3)
	BulkB9(3)
	BulkB11(3)
	BulkC1(6)
	BulkC3(6)
}
