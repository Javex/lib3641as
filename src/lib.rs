//! Driver for the 3641AS seven segment display with four digits.
//!
//! This module contains the code to display numbers on the 3641AS display. It supports 4-digit
//! integers, floating point numbers with single precision after the decimal point and can also
//! display a trailing character after a floating point number.
//!
//! The display is constructed using a [SegmentConfiguration] which provides the 8 pins connected
//! to the 7 segments and the decimal point. It also takes an array of 4 digit pins. The segment
//! pins are set to high when active while the digit pins are set to low when active. Once you have
//! created an object, set your desired output and then call [SevenSegment::tick] rapidly. See
//! [SevenSegment] for more instructions.
//!

#![no_std]
use core::fmt::Display;

use embedded_hal::digital::OutputPin;

/// Configure the 7 segments of the display by mapping which GPIO pin is connected to which segment.
/// The letters represent the segment as labelled.
/// ```text
/// ┌────A────┐
/// │         │
/// F         B
/// │         │
/// ├────G────┤
/// │         │
/// E         C
/// │         │
/// └────D────┘  ⬤ DP
/// ```
///
/// Fields below are lower-case versions of the segment names above. Segment names correspond to
/// segment names in the [3641AS datasheet](https://www.xlitx.com/datasheet/3641AS.pdf).
pub struct SegmentConfiguration<P: OutputPin> {
    pub a: P,
    pub b: P,
    pub c: P,
    pub d: P,
    pub e: P,
    pub f: P,
    pub g: P,
    pub dp: P,
}

#[derive(Debug)]
pub enum SegmentError<P: OutputPin> {
    DigitTooLarge(u8),
    NumberTooLarge(u16),
    FloatTooLarge(f32),
    FloatWithCharTooLarge(f32),
    InvalidDisplayIndex(u8),
    DigitalError(P::Error),
}

impl<P: OutputPin> Display for SegmentError<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DigitTooLarge(digit) => write!(f, "Digit must be between 0 and 9, not {}", digit),
            Self::NumberTooLarge(num) => {
                write!(f, "Number cannot be larger than 9999, too large: {}", num)
            }
            Self::FloatTooLarge(num) => {
                write!(f, "Number cannot be larger than 999.9, too large: {}", num)
            }
            Self::FloatWithCharTooLarge(num) => {
                write!(
                    f,
                    "Number cannot be larger than 99.9 with trailing character, too large: {}",
                    num
                )
            }
            Self::InvalidDisplayIndex(idx) => write!(
                f,
                "Display index must be between 0 and 3, invalid index: {}",
                idx
            ),
            Self::DigitalError(err) => write!(f, "Error changing pin output: {:?}", err),
        }
    }
}

// Each entry represents a digit based on the index of the array (e.g. index 0 is for number 0).
// The bit pattern in each entry represents which segments are turned on. The highest order bit is
// the segment G, then goes through the letters to A, so GFEDCBA. Thus, A is
// the lowest bit in this pattern.
const PATTERNS: [u8; 10] = [
    //GFEDCBA
    0b0111111, // 0
    0b0000110, // 1
    0b1011011, // 2
    0b1001111, // 3
    0b1100110, // 4
    0b1101101, // 5
    0b1111101, // 6
    0b0000111, // 7
    0b1111111, // 8
    0b1101111, // 9
];

// Show a single number after the decimal point
const DECIMAL_LOCATION: u8 = 1;

/// Display a character on the display. Only a limited set of characters are supported.
#[derive(Copy, Clone)]
pub enum DisplayChar {
    C,
    H,
}

impl DisplayChar {
    fn pattern(&self) -> u8 {
        match self {
            Self::C => 0b0111001,
            Self::H => 0b1110110,
        }
    }
}

#[derive(Copy, Clone)]
enum DisplayNumber {
    Integer(u16),
    Float(f32),
    FloatWithChar { number: f32, char: DisplayChar },
}

/// Drive a 3641AS seven segment display from this object.
///
/// Create a new instance using [SevenSegment::new]. Then set the desired value to be displayed
/// using one of the following options:
///
/// - [SevenSegment::show] for a 4-digit integer (e.g. `1234`)
/// - [SevenSegment::show_float] for a floating point number with three leading numbers and one
///   after the decimal point (e.g. `123.4`)
/// - [SevenSegment::show_float_with_char] for a floating point number with two leading numbers, one
///   after the decimal point and a trailing character (e.g. `12.3C`).
///
/// Then show this number by rapidly calling [SevenSegment::tick]. Since the display only provides
/// the ability to show segments for one number at a time, it's necessary to use multiplexing by
/// rapidly turning showing each digit, one at a time. At a high enough frequency this means the
/// human eye can't perceive the difference and it looks like a static display. Calling
/// [SevenSegment::tick] every 2ms should be enough.
pub struct SevenSegment<P: OutputPin> {
    /// Segments in order from A to G plus DP
    segments: [P; 8],
    /// Four digits, lowest index corresponds to lowest digit
    digits: [P; 4],
    /// The number to display, if any
    number: Option<DisplayNumber>,
    /// Internal state for the display to track which digit to display on the next tick
    current_digit: u8,
}

impl<P: OutputPin> SevenSegment<P> {
    pub fn new(segments: SegmentConfiguration<P>, digits: [P; 4]) -> Result<Self, P::Error> {
        let segments = [
            segments.a,
            segments.b,
            segments.c,
            segments.d,
            segments.e,
            segments.f,
            segments.g,
            segments.dp,
        ];
        let mut display = Self {
            segments,
            digits,
            number: None,
            current_digit: 0,
        };
        // Set pins to their initial state (no segments are lit up)
        for digit in display.digits.iter_mut() {
            digit.set_high()?;
        }
        for segment in display.segments.iter_mut() {
            segment.set_low()?;
        }
        Ok(display)
    }

    /// Call this method at a high frequency (e.g. every 2ms) to ensure all digits are displayed
    /// correctly. Call it at a low frequency and numbers will display separately.
    pub fn tick(&mut self) -> Result<(), SegmentError<P>> {
        self.show_digit(self.current_digit)?;
        // Increment the index by 1, wrapping around to 0. This means each time the next index of
        // the display is updated.
        self.current_digit = (self.current_digit + 1) % 4;
        Ok(())
    }

    /// Set the number to show on the display. This function only stores the value to be shown. The
    /// display needs to be updated at high frequency to show each digit. The maximum number that
    /// the display can show is `9999` and larger numbers will return an error.
    pub fn show(&mut self, number: u16) -> Result<(), SegmentError<P>> {
        if number > 9999 {
            return Err(SegmentError::NumberTooLarge(number));
        }
        self.number = Some(DisplayNumber::Integer(number));
        return Ok(());
    }

    /// Show a floating number with a single digit precision after the decimal point. The maximum
    /// number is `999.9` and larger numbers will return an error.
    pub fn show_float(&mut self, number: f32) -> Result<(), SegmentError<P>> {
        if number > 999.9 {
            return Err(SegmentError::FloatTooLarge(number));
        }
        self.number = Some(DisplayNumber::Float(number));
        return Ok(());
    }

    /// Show a floating number like [SevenSegment::show_float], but with a trailing character and
    /// one less digit. Maximum number is `99.9`. Supported characters are available in
    /// [DisplayChar].
    pub fn show_float_with_char(
        &mut self,
        number: f32,
        char: DisplayChar,
    ) -> Result<(), SegmentError<P>> {
        if number > 99.9 {
            return Err(SegmentError::FloatTooLarge(number));
        }
        self.number = Some(DisplayNumber::FloatWithChar { number, char });
        Ok(())
    }

    // Show the correct digit for the given display_index.
    fn show_digit(&mut self, display_index: u8) -> Result<(), SegmentError<P>> {
        if (display_index as usize) >= self.digits.len() {
            return Err(SegmentError::InvalidDisplayIndex(display_index));
        }

        // Clear the current number
        // This avoids "ghosting" where another digit's segments still light up a little
        // bit.
        self.blank()?;
        // Turn off all digits except the current one
        for (i, pin) in self.digits.iter_mut().enumerate() {
            if i == (display_index as usize) {
                pin.set_low()
            } else {
                pin.set_high()
            }
            .map_err(|e| SegmentError::DigitalError(e))?;
        }

        // Check if we have a floating number with a trailing character.
        // If that's the case we need to shift everything by one: The lowest index is actually the
        // character so the lowest digit is the first index and so on.
        let display_index = match self.number {
            Some(DisplayNumber::FloatWithChar { number: _, char }) => {
                if display_index == 0 {
                    // If we're on the lowest index we can stop here. We know to display the
                    // character pattern and not a number.
                    return self.display_pattern(char.pattern());
                } else {
                    // We have a trailing character and need to display a number, so shift the
                    // index by 1 so we fetch the right digit.
                    display_index - 1
                }
            }
            // Regular float or integer, no trailing character, keep index as-is
            _ => display_index,
        };

        // Display a fixed point for floating numbers
        // If a character is displayed, the decimal point will be shifted automatically because we
        // reduced the index by one above
        if display_index == DECIMAL_LOCATION
            && (matches!(self.number, Some(DisplayNumber::Float(_)))
                || matches!(self.number, Some(DisplayNumber::FloatWithChar { .. })))
        {
            self.segments[7]
                .set_high()
                .map_err(SegmentError::DigitalError)?;
        }

        // Find out which number we need to display
        let digit = self.get_digit(display_index);
        match digit {
            // None means leading zero and we show a blank
            None => self.blank()?,
            // Some means we have a number to display
            Some(digit) => self.display(digit)?,
        }
        Ok(())
    }

    // Helper method to find out which digit to display based on the number and display index
    fn get_digit(&self, digit: u8) -> Option<u8> {
        // If no number is set all digits are blank
        let number = self.number?;
        let number = match number {
            DisplayNumber::Integer(number) => number,
            // Multiplying by number makes an integer-equivalent.
            // Decimal point is displayed at a fixed spot, showing e.g. 12.3
            // Thus, multiplying by 10 ** DECIMAL_LOCATION here puts the number in the right spot
            // for the decimal place
            // For the FloatWithChar it's the same deal, because the shift left to make room for
            // the character happens elsewhere. The same digit is still returned correctly here.
            DisplayNumber::Float(number) | DisplayNumber::FloatWithChar { number, char: _ } => {
                (number * (10u16.pow(DECIMAL_LOCATION as u32) as f32)) as u16
            }
        };

        let divisor = 10u16.pow(digit as u32);
        // The divisor is 1, 10, 100, or 1,000. By checking if it is larger than the number, we can
        // determine if the value is a leading zero.
        // For example, if the value is 25 and we're displaying the 3rd index (digit), the divisor
        // would be 100 and detect that we have a leading zero.
        // The None case can then be handled by the caller to show a blank instead of zero.
        if divisor > number {
            None
        } else {
            Some(((number / divisor) % 10) as u8)
        }
    }

    // Show the given digit by lighting up all corresponding segments. Expects that the correct
    // digit pin has been set already and will just set the correct segments to high.
    fn display(&mut self, digit: u8) -> Result<(), SegmentError<P>> {
        let pattern = PATTERNS
            .get(digit as usize)
            .ok_or(SegmentError::DigitTooLarge(digit))?;
        self.display_pattern(*pattern)
    }

    fn display_pattern(&mut self, pattern: u8) -> Result<(), SegmentError<P>> {
        // Skip the last segment because it's the decimal point. It's not part of the pattern
        for (i, pin) in self.segments[..7].iter_mut().enumerate() {
            // The bit sequence (pattern) is selected based on the digit or character to display.
            // For each
            // segment, it has a 0 or a 1 to indicate whether this segment needs to light up for
            // the selected pattern (digit / character).
            // Since the lowest bit in the pattern corresponds to the first segment in the array,
            // the array index (i) is equal to the bit which decides if this segment needs to light
            // up or not.
            // By shifting the pattern down by the index, we make sure that the least significant
            // bit (LSB) corresponds to the segment (array index). Then remove all higher bits
            // using "& 1". The result is either a 1 or a 0 to say whether this segment needs to
            // light up or not.
            if (pattern >> i) & 1 == 1 {
                pin.set_high()
            } else {
                pin.set_low()
            }
            .map_err(|e| SegmentError::DigitalError(e))?;
        }
        Ok(())
    }

    // Clear all segments, showing nothing at all
    fn blank(&mut self) -> Result<(), SegmentError<P>> {
        for pin in self.segments.iter_mut() {
            pin.set_low().map_err(|e| SegmentError::DigitalError(e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::cell::Cell;
    use std::rc::Rc;

    use embedded_hal::digital::ErrorType;

    use super::*;

    #[derive(Clone, Debug)]
    struct MockOutputPin {
        state: Rc<Cell<bool>>,
    }

    impl ErrorType for MockOutputPin {
        type Error = embedded_hal::digital::ErrorKind;
    }

    impl OutputPin for MockOutputPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.state.set(false);
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.state.set(true);
            Ok(())
        }
    }

    fn make_pin() -> (MockOutputPin, Rc<Cell<bool>>) {
        let state = Rc::new(Cell::new(false));
        let pin = MockOutputPin {
            state: Rc::clone(&state),
        };
        (pin, state)
    }

    struct DisplayFixture {
        display: SevenSegment<MockOutputPin>,
        seg_states: [Rc<Cell<bool>>; 8],
        dig_states: [Rc<Cell<bool>>; 4],
    }

    impl DisplayFixture {
        fn new() -> Self {
            let (seg_a, seg_a_s) = make_pin();
            let (seg_b, seg_b_s) = make_pin();
            let (seg_c, seg_c_s) = make_pin();
            let (seg_d, seg_d_s) = make_pin();
            let (seg_e, seg_e_s) = make_pin();
            let (seg_f, seg_f_s) = make_pin();
            let (seg_g, seg_g_s) = make_pin();
            let (seg_dp, seg_dp_s) = make_pin();
            let (dig0, dig0_s) = make_pin();
            let (dig1, dig1_s) = make_pin();
            let (dig2, dig2_s) = make_pin();
            let (dig3, dig3_s) = make_pin();

            let display = SevenSegment::new(
                SegmentConfiguration {
                    a: seg_a,
                    b: seg_b,
                    c: seg_c,
                    d: seg_d,
                    e: seg_e,
                    f: seg_f,
                    g: seg_g,
                    dp: seg_dp,
                },
                [dig0, dig1, dig2, dig3],
            )
            .unwrap();

            Self {
                display,
                seg_states: [
                    seg_a_s, seg_b_s, seg_c_s, seg_d_s, seg_e_s, seg_f_s, seg_g_s, seg_dp_s,
                ],
                dig_states: [dig0_s, dig1_s, dig2_s, dig3_s],
            }
        }

        fn assert_digit_active(&self, active: usize) {
            for (i, state) in self.dig_states.iter().enumerate() {
                if i == active {
                    assert!(!state.get(), "digit[{}] should be LOW (active)", i);
                } else {
                    assert!(state.get(), "digit[{}] should be HIGH (inactive)", i);
                }
            }
        }

        fn assert_segments_match_pattern(&self, pattern: u8) {
            for i in 0..7 {
                let expected = (pattern >> i) & 1 == 1;
                assert_eq!(
                    self.seg_states[i].get(),
                    expected,
                    "segment[{}] (bit {}) should be {}",
                    i,
                    i,
                    if expected { "HIGH" } else { "LOW" }
                );
            }
        }

        fn assert_dp(&self, high: bool) {
            assert_eq!(
                self.seg_states[7].get(),
                high,
                "DP should be {}",
                if high { "HIGH" } else { "LOW" }
            );
        }

        fn assert_blank(&self) {
            for (i, state) in self.seg_states.iter().enumerate() {
                assert!(!state.get(), "segment[{}] should be LOW (blank)", i);
            }
        }
    }

    #[test]
    fn test_display_1234() {
        let mut fixture = DisplayFixture::new();
        fixture.display.show(1234).unwrap();

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(0);
        fixture.assert_segments_match_pattern(PATTERNS[4]);
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(1);
        fixture.assert_segments_match_pattern(PATTERNS[3]);
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(2);
        fixture.assert_segments_match_pattern(PATTERNS[2]);
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(3);
        fixture.assert_segments_match_pattern(PATTERNS[1]);
        fixture.assert_dp(false);
    }

    #[test]
    fn test_display_float() {
        // 12.3 → internal value 123; decimal point at display_index 1
        let mut fixture = DisplayFixture::new();
        fixture.display.show_float(12.3).unwrap();

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(0);
        fixture.assert_segments_match_pattern(PATTERNS[3]);
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(1);
        fixture.assert_segments_match_pattern(PATTERNS[2]);
        fixture.assert_dp(true);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(2);
        fixture.assert_segments_match_pattern(PATTERNS[1]);
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(3);
        fixture.assert_blank();
    }

    #[test]
    fn test_display_float_with_char() {
        // 12.3 with trailing C; digit[0] = C, digits[1-3] = 3/2/1 (shifted), DP at digit[2]
        let mut fixture = DisplayFixture::new();
        fixture
            .display
            .show_float_with_char(12.3, DisplayChar::C)
            .unwrap();

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(0);
        fixture.assert_segments_match_pattern(DisplayChar::C.pattern());
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(1);
        fixture.assert_segments_match_pattern(PATTERNS[3]);
        fixture.assert_dp(false);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(2);
        fixture.assert_segments_match_pattern(PATTERNS[2]);
        fixture.assert_dp(true);

        fixture.display.tick().unwrap();
        fixture.assert_digit_active(3);
        fixture.assert_segments_match_pattern(PATTERNS[1]);
        fixture.assert_dp(false);
    }
}
