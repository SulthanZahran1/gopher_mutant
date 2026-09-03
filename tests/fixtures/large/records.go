package large

type Record struct {
	value int
	label string
}

func (r Record) Value() int {
	return r.value + 1
}

func (r Record) Enabled() bool {
	return r.value > 0 && r.value <= 10
}

type Valuer interface{ Value() int }

func ReadValue(v Valuer) int {
	return v.Value()
}

func RecordExact(r Record) int {
	return r.value * 2
}

func RecordBare(r Record) int {
	return r.value - 1
}
