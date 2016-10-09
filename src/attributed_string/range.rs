use super::element::Element;

struct EltRangeValue {
    Boundary{elt_index: usize};
    Middle{elt_index: usize, offset: usize};
}

impl EltRangeValue {
    pub fn new(elt_index: usize, offset: usize) -> Self {
        match offset {
            0 => Boundary{elt_index: elt_index},
            _ => Middle{elt_index: elt_index, offset: offset}
        }
    }
}

pub struct EltRange {
    start: EltRangeValue,
    end: EltRangeValue,
}

impl EltRange {
    pub fn excluding_boundary_attrs(elements: &[Element], start: usize, len: usize) -> Self {
        let elements = elements.iter();
        let char_index:      usize = 0;
        let elt_index:       usize = 0;

        let start_elt_index: usize = 0;
        let start_offset:    usize = 0;
        let stop_elt_index:  usize = 0;
        let stop_offset:     usize = 0;

        while let elt = elements.next() {
            let elt_len = elt.len();
            let stop = start + len;

            if char_index <= start && char_index+elt_len > start {
                start_elt_index = elt_index;
                start_offset = start - char_index;
            } else if char_index <= stop && char_index+elt_len > stop {
                stop_elt_index = elt_index;
                stop_offset = stop - char_index;
                break;
            }

            char_index += elt_len;
            elt_index += 1;
        }
        new(start_elt_index, start_offset, stop_elt_index, stop_offset)
    }


    fn new(start_elt_index: usize, start_offset: usize, stop_elt_index: usize, stop_offset: usize) -> Self {
        let start = EltRangeValue::new(start_elt_index, start_offset);
        let stop = EltRangeValue::new(stop_elt_index, stop_offset);
        EltRange{start: start, stop: stop}
    }
}
