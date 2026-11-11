//! Value axes that count things: every tick on a whole number.
//!
//! GPUI Kit's charts space their ticks evenly between zero and the largest
//! value, so a maximum of 5 with five ticks labels 1.25, 2.5 and 3.75, which
//! rounds to a misleading "1, 2, 4". These pick the tick count, or the top of
//! the axis, so that the steps are whole.

/// For a chart whose axis ends at `max` (a bar chart's ends at its largest
/// value): the most ticks, up to `most`, whose steps divide `max`. Falls back
/// to two ticks, zero and `max`.
pub fn whole_ticks(max: u32, most: usize) -> usize {
    (2..=most.max(2))
        .rev()
        .find(|n| max > 0 && max.is_multiple_of(*n as u32 - 1))
        .unwrap_or(2)
}

/// For a chart whose axis can be set (an area chart's `y_domain`): a round top
/// at or above `max`, in steps of 1, 2 or 5 times a power of ten, with at most
/// `most` ticks. Returns the top and the tick count.
pub fn round_axis(max: u32, most: usize) -> (u32, usize) {
    let intervals = most.max(2) as u32 - 1;
    let mut magnitude = 1u32;
    loop {
        for factor in [1, 2, 5] {
            let step = factor * magnitude;
            let steps = max.div_ceil(step).max(1);
            if steps <= intervals {
                return (steps * step, steps as usize + 1);
            }
        }
        magnitude *= 10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_ticks_divide_the_maximum() {
        assert_eq!(whole_ticks(5, 6), 6); // 0, 1, 2, 3, 4, 5
        assert_eq!(whole_ticks(8, 6), 5); // 0, 2, 4, 6, 8
        assert_eq!(whole_ticks(7, 6), 2); // 0, 7
        assert_eq!(whole_ticks(0, 6), 2);
    }

    #[test]
    fn round_axes_end_on_a_round_number() {
        assert_eq!(round_axis(287, 6), (300, 4)); // 0, 100, 200, 300
        assert_eq!(round_axis(74, 6), (80, 5)); // 0, 20, 40, 60, 80
        assert_eq!(round_axis(5, 6), (5, 6));
        assert_eq!(round_axis(0, 6), (1, 2));
        assert_eq!(round_axis(1000, 6), (1000, 6)); // 0, 200, …, 1000
    }
}
