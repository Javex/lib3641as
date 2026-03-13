# 3641AS Seven Segment Display

An embedded Rust library to drive a 7 segment, 4 digit display (the 3641AS
display). See module documentation for more details.

Here's an example:

```rust
    let segment_config = SegmentConfiguration {
        a: new_output_pin(peripherals.GPIO19),
        b: new_output_pin(peripherals.GPIO17),
        c: new_output_pin(peripherals.GPIO2),
        d: new_output_pin(peripherals.GPIO4),
        e: new_output_pin(peripherals.GPIO0),
        f: new_output_pin(peripherals.GPIO18),
        g: new_output_pin(peripherals.GPIO5),
        dp: new_output_pin(peripherals.GPIO16),
    };
    // Start with the lowest digit as SevenSegment expects this array order.
    let digits = [
        new_output_pin(peripherals.GPIO32),
        new_output_pin(peripherals.GPIO33),
        new_output_pin(peripherals.GPIO25),
        new_output_pin(peripherals.GPIO26),
    ];
    let display = match SevenSegment::new(segment_config, digits).unwrap();
    display.show(1234).unwrap();
    let delay = Delay::new();
    loop {
        display.tick();
        delay.delay_millis(2);
    }
```

Use a `180 Ohm` resistor (anything from `150 - 220` should work just fine) on
each of the segments and adjust the GPIO pins based on your wiring.
