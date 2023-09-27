use core::fmt;
use std::{net::Ipv4Addr, thread, time::Duration, process::Command};

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*
};

use embedded_graphics_simulator::{BinaryColorTheme, SimulatorDisplay, Window, OutputSettingsBuilder};
use u8g2_fonts::{fonts, FontRenderer, types::{HorizontalAlignment, VerticalPosition, FontColor}};

use wifiscanner;

use local_ip_address::local_ip;

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

impl Percent{
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
fn main() -> Result<(), core::convert::Infallible> {
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    let mut display = SimulatorDisplay::<BinaryColor>::new(Size::new(128, 64));

    let font1 = FontRenderer::new::<fonts::u8g2_font_inr24_mr>();
    let font1_small = FontRenderer::new::<fonts::u8g2_font_inb16_mr>();
    let font2 = FontRenderer::new::<fonts::u8g2_font_8x13_mr>();
   
    font1_small.render_aligned(
        "Beltpack\nIntercom",
        display.bounding_box().center().x_axis() + Point::new(0, 2),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::Transparent(BinaryColor::On),
        &mut display
        )
        .unwrap();
    
    font2.render_aligned(
        (String::from("SW: ") + VERSION).as_str(),
        display.bounding_box().center().x_axis() + Point::new(0,64),
        VerticalPosition::Bottom,
        HorizontalAlignment::Center,
        FontColor::Transparent(BinaryColor::On),
        &mut display
        )
        .unwrap();

    let output_settings = OutputSettingsBuilder::new()
        .theme(BinaryColorTheme::OledWhite)
        .build();
    let mut window = Window::new("Hello World", &output_settings);
    window.update(&mut display);
    
    thread::sleep(Duration::from_secs(3));
    display.clear(BinaryColor::Off).unwrap();


    let text = "CAM 2";
    let target_user_1: User = User::new(text).unwrap(); 


    let local_ip = local_ip().unwrap();
    println!("{:?}", local_ip);
    let blank_ip = "1.1.1.1";
    let formated_ip: Ipv4Addr = blank_ip.parse::<Ipv4Addr>().unwrap();

    let p = Command::new("sh")
        .arg("-c")
        .arg("nmcli dev wifi list | awk '/\\*/{if (NR!=1) {print $8}}'")
        .output()
        .expect("failed to exectue po");
    
    println!("{:?}", String::from_utf8_lossy(&p.stdout).trim_end());

    let percent: Percent = Percent::new(String::from_utf8_lossy(&p.stdout).trim_end()).unwrap();
    
    font2.render_aligned(
        (percent.to_string() + "%").as_str(),
        display.bounding_box().top_left + Point::new(0, 1),
        VerticalPosition::Top,
        HorizontalAlignment::Left,
        FontColor::Transparent(BinaryColor::On),
        &mut display
        )
        .unwrap();

    font1.render_aligned(
        target_user_1.to_string().as_str(),
        display.bounding_box().center() + Point::new(2, 2),
        VerticalPosition::Center,
        HorizontalAlignment::Center,
        FontColor::Transparent(BinaryColor::On),
        &mut display,
        )
        .unwrap();

    font2.render_aligned(
        "TALK TO",
        display.bounding_box().center() + Point::new(64, -30),
        VerticalPosition::Top,
        HorizontalAlignment::Right,
        FontColor::Transparent(BinaryColor::On),
        &mut display,
        )
        .unwrap();

    font2.render_aligned(
        local_ip.to_string().as_str(),
        Point::new(display.bounding_box().center().x, 64),
        VerticalPosition::Bottom,
        HorizontalAlignment::Center,
        FontColor::Transparent(BinaryColor::On),
        &mut display,
        )
        .unwrap();

    window.show_static(&mut display);
    Ok(())
}
