use core::fmt;
use std::{
    net::IpAddr,
    path::{self, Path},
    process::Command,
    thread,
    time::Duration,
};

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, PrimitiveStyleBuilder, Sector},
};
use embedded_graphics_core::primitives::Rectangle;
use gpio_cdev::{Chip, LineRequestFlags};
use linux_embedded_hal::{CdevPin, I2cdev};
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::{DisplayConfig, I2CInterface},
    rotation::DisplayRotation,
    size::{DisplaySize, DisplaySize128x64},
    I2CDisplayInterface, Ssd1306,
};
use u8g2_fonts::{
    fonts,
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

use local_ip_address::local_ip;

use configparser::ini::Ini;

enum Button {
    Power,
    Ptt1,
    Ptt2,
    Unknown,
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Button::Power => write!(f, "Power"),
            Button::Ptt1 => write!(f, "PTT1"),
            Button::Ptt2 => write!(f, "PTT2"),
            Button::Unknown => write!(f, "No Signal"),
        }
    }
}

#[derive(Debug)]
struct User(String);

impl User {
    fn new(value: &str) -> Result<Self, String> {
        if value.len() <= 6 && value.chars().all(|c| c.is_ascii()) {
            Ok(User(value.to_string()))
        } else {
            Err("Invalid Input".to_string())
        }
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
struct Percent(u8);

impl Percent {
    fn new(value: &str) -> Result<Self, String> {
        let new_value: u8 = value.parse().unwrap();
        if new_value <= 100 {
            Ok(Percent(new_value.into()))
        } else {
            Err("Invalid Input".to_string())
        }
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
struct Error(u8);

impl Error {
    fn new(value: &str) -> Result<Self, String> {
        if value.len() == 4 {
            let modified = value[..2].to_owned() + &value[3..];
            if modified.chars().all(|c| c.is_digit(6)) {
                Ok(Error(u8::from_str_radix(modified.as_str(), 6).unwrap()))
            } else {
                Err("Invalid input".to_string())
            }
        } else {
            Err("Invalid input".to_string())
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = vec![];
        let mut n: u8 = self.0;
        loop {
            let m = n % 6;
            n = n / 6;

            // will panic if you use a bad radix (< 2 or > 36).
            result.push(std::char::from_digit(m.into(), 6).unwrap());
            if n == 0 {
                break;
            }
        }
        let mut v: String = result.into_iter().rev().collect();
        v.truncate(2);
        return write!(f, "{}", v);
    }
}

fn handle_error<T, E>(
    result: Result<T, E>,
    display: &mut Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    font2: &FontRenderer,
) where
    E: std::fmt::Display,
{
    match result {
        Ok(_) => {}
        Err(err) => {
            let fill = PrimitiveStyleBuilder::new()
                .stroke_color(BinaryColor::On)
                .fill_color(BinaryColor::On)
                .build();

            Rectangle::new(Point::new(0, 50), Size::new(128, 14))
                .into_styled(fill)
                .draw(&mut *display)
                .unwrap();

            font2
                .render_aligned(
                    (String::from("EPRROR: ") + err.to_string().as_str()).as_str(),
                    Point::new(display.bounding_box().center().x, 64),
                    VerticalPosition::Bottom,
                    HorizontalAlignment::Center,
                    FontColor::WithBackground {
                        fg: BinaryColor::Off,
                        bg: BinaryColor::On,
                    },
                    &mut *display,
                )
                .unwrap();
        }
    }
}

fn main() -> Result<(), core::convert::Infallible> {
    let i2c = I2cdev::new::<&Path>(Path::new("/dev/i2c-0").as_ref()).unwrap();
    let interface = I2CDisplayInterface::new(i2c);

    let mut config = Ini::new();
    config.load("config.ini").unwrap();
    let current_user: User =
        User::new(config.get("config", "device_name").unwrap().as_str()).unwrap();
    let target_user_1: User =
        User::new(config.get("config", "target_1").unwrap().as_str()).unwrap();
    let target_user_2: User =
        User::new(config.get("config", "target_2").unwrap().as_str()).unwrap();

    //let mut display = SimulatorDisplay::<BinaryColor>::new(Size::new(128, 64));
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap();

    let font1 = FontRenderer::new::<fonts::u8g2_font_inr24_mr>();
    let font1_small = FontRenderer::new::<fonts::u8g2_font_inb16_mr>();
    let font2 = FontRenderer::new::<fonts::u8g2_font_8x13_mr>();

    // TODO: Please change these to use the correct GPIO lines
    let mut gpiochip = Chip::new("/dev/gpiochip0").unwrap();
    let power_gpio = gpiochip.get_line(17).unwrap();
    let ptt1_gpio = gpiochip.get_line(18).unwrap();
    let ptt2_gpio = gpiochip.get_line(19).unwrap();

    let power_handle = power_gpio
        .request(LineRequestFlags::INPUT, 1, "my-gpio")
        .unwrap();
    let ptt1_handle = ptt1_gpio
        .request(LineRequestFlags::INPUT, 1, "my-gpio")
        .unwrap();
    let ptt2_handle = ptt2_gpio
        .request(LineRequestFlags::INPUT, 1, "my-gpio")
        .unwrap();

    boot_screen(&mut display, &font1_small, &font2);
    display.flush().unwrap();

    thread::sleep(Duration::from_secs(3));
    display.clear(BinaryColor::Off).unwrap();

    let local_ip_addr = local_ip().unwrap();

    let p = Command::new("sh")
        .arg("-c")
        .arg("nmcli dev wifi list | awk '/\\*/{if (NR!=1) {print $8}}'")
        .output()
        .expect("failed to exectue po");

    let percent: Percent = Percent::new(String::from_utf8_lossy(&p.stdout).trim_end()).unwrap();

    signal_display(&mut display, &font2, percent);

    name_display(&mut display, &font1, &font2, &target_user_1, false);

    ip_display(&mut display, &font2, local_ip_addr);
    display.flush().unwrap();

    let mut counter = 0;
    let mut secs: u8 = 0;

    loop {
        if power_handle.get_value().unwrap() == 1 {
            power_display(&mut display, &secs.into(), 9f32);
            display.flush().unwrap();
            if secs >= 10 {
                break;
            }
            secs += 1;
            thread::sleep(Duration::from_millis(100));
            continue;
        } else if secs != 0 {
            secs = 0;
            display.clear(BinaryColor::Off).unwrap();
            counter = 10;
        }
        if ptt1_handle.get_value().unwrap() == 1 {
            name_display(&mut display, &font1, &font2, &target_user_1, true);
            display.flush().unwrap();
        } else if ptt2_handle.get_value().unwrap() == 1 {
            name_display(&mut display, &font1, &font2, &target_user_2, true);
            display.flush().unwrap();
        } else {
            name_display(&mut display, &font1, &font2, &current_user, false);
            display.flush().unwrap();
        }

        if counter == 10 || secs != 0 {
            let p = Command::new("sh")
                .arg("-c")
                .arg("nmcli dev wifi list | awk '/\\*/{if (NR!=1) {print $8}}'")
                .output()
                .expect("failed to exectue po");

            let percent: Percent =
                Percent::new(String::from_utf8_lossy(&p.stdout).trim_end()).unwrap();

            signal_display(&mut display, &font2, percent);
            let local_ip_addr = local_ip().unwrap();
            ip_display(&mut display, &font2, local_ip_addr);

            display.flush().unwrap();
            counter = 0;
        }

        counter += 1;

        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn boot_screen(
    display: &mut Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    font1_small: &FontRenderer,
    font2: &FontRenderer,
) {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    font1_small
        .render_aligned(
            "Beltpack\nIntercom",
            display.bounding_box().center().x_axis() + Point::new(0, 2),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut *display,
        )
        .unwrap();

    font2
        .render_aligned(
            (String::from("SW: ") + VERSION).as_str(),
            display.bounding_box().center().x_axis() + Point::new(0, 64),
            VerticalPosition::Bottom,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut *display,
        )
        .unwrap();
}

fn name_display(
    display: &mut Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    font1: &FontRenderer,
    font2: &FontRenderer,
    user: &User,
    talking: bool,
) {
    let clear = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::Off)
        .fill_color(BinaryColor::Off)
        .build();

    Rectangle::new(Point::new(128 - 58, 0), Size::new(58, 14))
        .into_styled(clear)
        .draw(&mut *display)
        .unwrap();

    Rectangle::new(Point::new(0, 16), Size::new(128, 33))
        .into_styled(clear)
        .draw(&mut *display)
        .unwrap();

    font1
        .render_aligned(
            user.to_string().as_str(),
            display.bounding_box().center() + Point::new(2, 2),
            VerticalPosition::Center,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut *display,
        )
        .unwrap();

    if talking {
        font2
            .render_aligned(
                "TALK TO",
                display.bounding_box().center() + Point::new(64, -30),
                VerticalPosition::Top,
                HorizontalAlignment::Right,
                FontColor::Transparent(BinaryColor::On),
                &mut *display,
            )
            .unwrap();
    }
}

fn signal_display(
    display: &mut Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    font2: &FontRenderer,
    percent: Percent,
) {
    let clear = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::Off)
        .fill_color(BinaryColor::Off)
        .build();

    Rectangle::new(Point::new(0, 0), Size::new(34, 14))
        .into_styled(clear)
        .draw(&mut *display)
        .unwrap();

    font2
        .render_aligned(
            (percent.to_string() + "%").as_str(),
            display.bounding_box().top_left + Point::new(0, 1),
            VerticalPosition::Top,
            HorizontalAlignment::Left,
            FontColor::Transparent(BinaryColor::On),
            &mut *display,
        )
        .unwrap();
}

fn ip_display(
    display: &mut Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    font2: &FontRenderer,
    ip: IpAddr,
) {
    let clear = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::Off)
        .fill_color(BinaryColor::Off)
        .build();

    Rectangle::new(Point::new(0, 50), Size::new(128, 14))
        .into_styled(clear)
        .draw(&mut *display)
        .unwrap();

    font2
        .render_aligned(
            ip.to_string().as_str(),
            Point::new(display.bounding_box().center().x, 64),
            VerticalPosition::Bottom,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut *display,
        )
        .unwrap();
}

fn power_display(
    display: &mut Ssd1306<
        I2CInterface<I2cdev>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    currenta: &f32,
    maxa: f32,
) {
    display.clear(BinaryColor::Off).unwrap();
    let current = currenta + 1f32;
    let max = maxa + 1f32;

    let outline = Circle::with_center(display.bounding_box().center(), 48)
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2));
    Sector::from_circle(
        outline.primitive,
        Angle::from_degrees(0.0),
        Angle::from_degrees(360.0f32 * (current / max)),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(&mut *display)
    .unwrap();
    outline.draw(&mut *display).unwrap();
}

// fn test(
//     display: &mut Ssd1306<
//     I2CInterface<I2cdev>,
//     DisplaySize128x64,
//     BufferedGraphicsMode<DisplaySize128x64>,
//     >,
//     font1_small: &FontRenderer,
//     font2: &FontRenderer,
//     ) -> (Button, Button, Button) {
//     display.clear(BinaryColor::Off).unwrap();
//     let mut power = Button::Unknown;
//     let mut ptt1 = Button::Unknown;
//     let mut ptt2 = Button::Unknown;
//     let mut secs = 0;
//     font1_small
//         .render_aligned(
//             "TEST",
//             display.bounding_box().center(),
//             VerticalPosition::Center,
//             HorizontalAlignment::Center,
//             FontColor::Transparent(BinaryColor::On),
//             &mut *display,
//             )
//         .unwrap();
//     display.flush().unwrap();
//     thread::sleep(Duration::from_secs(1));
//     let device_state = DeviceState::new();
//     display.clear(BinaryColor::Off).unwrap();
//     font1_small
//         .render_aligned(
//             "POWER",
//             display.bounding_box().center(),
//             VerticalPosition::Center,
//             HorizontalAlignment::Center,
//             FontColor::Transparent(BinaryColor::On),
//             &mut *display,
//             )
//         .unwrap();
//     display.flush().unwrap();
//     loop {
//         let keys: Vec<Keycode> = device_state.get_keys();
//
//         if keys.contains(&Keycode::Escape) {
//             power = Button::Power;
//         } else if keys.contains(&Keycode::Left) {
//             power = Button::Ptt1;
//         } else if keys.contains(&Keycode::Right) {
//             power = Button::Ptt2;
//         }
//
//         if !matches!(power, Button::Unknown)
//             && !keys.contains(&Keycode::Escape)
//                 && !keys.contains(&Keycode::Left)
//                 && !keys.contains(&Keycode::Right)
//                 {
//                     break;
//                 }
//
//         if secs >= 50 {
//             break;
//         }
//
//         thread::sleep(Duration::from_millis(100));
//         secs += 1;
//     }
//     thread::sleep(Duration::from_secs(1));
//     secs = 0;
//     display.clear(BinaryColor::Off).unwrap();
//     font1_small
//         .render_aligned(
//             "PTT 1",
//             display.bounding_box().center(),
//             VerticalPosition::Center,
//             HorizontalAlignment::Center,
//             FontColor::Transparent(BinaryColor::On),
//             &mut *display,
//             )
//         .unwrap();
//     display.flush().unwrap();
//     loop {
//         let keys: Vec<Keycode> = device_state.get_keys();
//
//         if keys.contains(&Keycode::Left) {
//             ptt1 = Button::Ptt1;
//         } else if keys.contains(&Keycode::Escape) {
//             ptt1 = Button::Power;
//         } else if keys.contains(&Keycode::Right) {
//             ptt1 = Button::Ptt2;
//         }
//
//         if !matches!(ptt1, Button::Unknown)
//             && !keys.contains(&Keycode::Escape)
//                 && !keys.contains(&Keycode::Left)
//                 && !keys.contains(&Keycode::Right)
//                 {
//                     break;
//                 }
//         if secs >= 50 {
//             break;
//         }
//
//         thread::sleep(Duration::from_millis(100));
//         secs += 1;
//     }
//
//     secs = 0;
//     display.clear(BinaryColor::Off).unwrap();
//     font1_small
//         .render_aligned(
//             "PTT 2",
//             display.bounding_box().center(),
//             VerticalPosition::Center,
//             HorizontalAlignment::Center,
//             FontColor::Transparent(BinaryColor::On),
//             &mut *display,
//             )
//         .unwrap();
//     display.flush().unwrap();
//     loop {
//         let keys: Vec<Keycode> = device_state.get_keys();
//
//         if keys.contains(&Keycode::Right) {
//             ptt2 = Button::Ptt2;
//         } else if keys.contains(&Keycode::Escape) {
//             ptt2 = Button::Power;
//         } else if keys.contains(&Keycode::Left) {
//             ptt2 = Button::Ptt1;
//         }
//
//         if !matches!(ptt2, Button::Unknown)
//             && !keys.contains(&Keycode::Escape)
//                 && !keys.contains(&Keycode::Left)
//                 && !keys.contains(&Keycode::Right)
//                 {
//                     break;
//                 }
//         if secs >= 50 {
//             break;
//         }
//
//         thread::sleep(Duration::from_millis(100));
//         secs += 1;
//     }
//
//     if matches!(power, Button::Power)
//         && matches!(ptt1, Button::Ptt1)
//             && matches!(ptt2, Button::Ptt2)
//             {
//                 display.clear(BinaryColor::Off).unwrap();
//                 font1_small
//                     .render_aligned(
//                         "ALL GOOD",
//                         display.bounding_box().center(),
//                         VerticalPosition::Center,
//                         HorizontalAlignment::Center,
//                         FontColor::Transparent(BinaryColor::On),
//                         &mut *display,
//                         )
//                     .unwrap();
//                 display.flush().unwrap();
//                 thread::sleep(Duration::from_secs(1));
//             } else {
//                 display.clear(BinaryColor::Off).unwrap();
//                 font1_small
//                     .render_aligned(
//                         "ERROR!",
//                         display.bounding_box().center(),
//                         VerticalPosition::Center,
//                         HorizontalAlignment::Center,
//                         FontColor::Transparent(BinaryColor::On),
//                         &mut *display,
//                         )
//                     .unwrap();
//                 display.flush().unwrap();
//                 thread::sleep(Duration::from_secs_f32(0.5));
//
//                 display.clear(BinaryColor::Off).unwrap();
//                 font2
//                     .render_aligned(
//                         format!("Power: {}\nPTT1: {}\nPTT2: {}", power, ptt1, ptt2).as_str(),
//                         display.bounding_box().center(),
//                         VerticalPosition::Center,
//                         HorizontalAlignment::Center,
//                         FontColor::Transparent(BinaryColor::On),
//                         &mut *display,
//                         )
//                     .unwrap();
//                 display.flush().unwrap();
//                 thread::sleep(Duration::from_secs(2));
//                 display.clear(BinaryColor::Off).unwrap();
//                 display.flush().unwrap();
//             }
//     (power, ptt1, ptt2)
// }
