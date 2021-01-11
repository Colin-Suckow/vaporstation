use core::num;

use bit_field::BitField;
use num_derive::FromPrimitive;

pub struct Gpu {
    vram: Vec<u16>,
    status_reg: u32,
    pixel_count: u32,
    enabled: bool,
    gp0_words_to_read: usize,
    gp0_buffer: [u32; 16],
    gp0_buffer_address: usize,

    texpage_x_base: u16,
    texpage_y_base: u16,

    draw_area_top_left_x: u16,
    draw_area_top_left_y: u16,
    draw_area_bottom_right_x: u16,
    draw_area_bottom_right_y: u16,
}

#[derive(FromPrimitive)]
enum RectangleSize {
    VariableSize,
    SinglePixel,
    EightSprite,
    SixteenSprite,
}

impl Gpu {
    pub fn new() -> Gpu {
        Gpu {
            vram: vec![0; (1_048_576 / 2)],
            status_reg: 0,
            pixel_count: 0,
            enabled: false,
            gp0_words_to_read: 0,
            gp0_buffer: [0; 16],
            gp0_buffer_address: 0,

            texpage_x_base: 0,
            texpage_y_base: 0,

            draw_area_top_left_x: 0,
            draw_area_top_left_y: 0,
            draw_area_bottom_right_x: 0,
            draw_area_bottom_right_y: 0,
        }
    }

    pub fn read_status_register(&self) -> u32 {
        self.status_reg
    }

    pub fn read_word_gp0(&mut self) -> u32 {
        0
    }

    pub fn send_gp0_command(&mut self, value: u32) {

        self.gp0_push(value);

        let command = self.gp0_buffer[0];

        match command.gp0_header() {
            0x0 => {
                //NOP
            }

            0x1 => {
                //Render Polygon

                // If the polygon is textured or gouraud shaded, lets just lock up the emulator.
                // I only want to test flat shaded polygons right now
                if command.get_bit(28) || command.get_bit(1) {
                    self.gp0_buffer_address = 1; //Prevent overflowing the buffer with more calls.
                    return;
                }

                let verts = if command.get_bit(27) {4} else {3};

                if self.gp0_buffer_address < verts {
                    // Not enough words for the command. Return early
                    return;
                }

                //Actually draw the polygon
                panic!("Tried to draw a polygon. I don't want to do this right now");
                
            }

            0x3 => {
                //Render Rectangle

                // If the rectangle is textured, lets just lock up the emulator.
                // I only want to test flat shaded rectangles right now
                if command.get_bit(26) {
                    self.gp0_buffer_address = 1; //Prevent overflowing the buffer with more calls.
                    return;
                }

                let size = (command >> 27) & 0x3;

                let length = 2 + if size == 0 {1} else {0};
                
                if self.gp0_buffer_address < length {
                    //Not enough commands
                    return;
                }

                match size {
                    0b01 => {
                        //Draw single pixel
                        let x = self.gp0_buffer[1] & 0xFFFF;
                        let y = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        let address = self.point_to_address(x, y) as usize;
                        self.vram[address] = (self.gp0_buffer[0] & 0x1FFFFFF) as u16;
                    }

                    0b0 => {
                        //Draw variable size
                        let x1 = self.gp0_buffer[1] & 0xFFFF;
                        let y1 = (self.gp0_buffer[1] >> 16) & 0xFFFF;
                        let x2 = self.gp0_buffer[2] & 0xFFFF;
                        let y2 = (self.gp0_buffer[2] >> 16) & 0xFFFF;

                        self.draw_solid_box(x1, y1, x2, y2, (self.gp0_buffer[0] & 0x1FFFFFF) as u16);
                    }

                    _ => {
                        //Lets do nothing with the others
                    }
                }
                
            }

            0x7 => {
                //Env commands
                match command.command() {
                    0xE1 => {
                        //Draw Mode Setting
                        //TODO I'm going to ignore everything but the texture page settings for now
                        self.texpage_x_base = ((command & 0xF) * 64) as u16;
                        self.texpage_y_base = if command.get_bit(4) {256} else {0};
                    }

                    0xE3 => {
                        //Set Drawing Area Top Left
                        self.draw_area_top_left_x = (command & 0x3FF) as u16;
                        self.draw_area_top_left_y = ((command >> 10) & 0x1FF) as u16;
                    }

                    0xE4 => {
                        //Set Drawing Area Bottom Right
                        self.draw_area_bottom_right_x = (command & 0x3FF) as u16;
                        self.draw_area_bottom_right_y = ((command >> 10) & 0x1FF) as u16;
                    }

                    0xE5 => {
                        //Set Drawing Offset
                        //TODO Implement. I'm too lazy right now
                    }
                    _ => panic!("Unknown GPU ENV command {:#X}", command.command())
                }
            }

            _ => panic!("unknown gp0 {:#X}!", command.gp0_header())
        }
        //Made it to the end, so the command must have been executed
        self.gp0_clear();
    }

    pub fn send_gp1_command(&mut self, command: u32) {
        match command.command() {
            0x0 => {
                //Reset GPU
                self.enabled = false;
                self.status_reg = 0;
                self.pixel_count = 0;
                self.vram = vec![0; 1_048_576 / 2];
            }

            0x6 => {
                //Horizontal Display Range
                //Ignore this one for now
            }

            0x10 => {
                //Get gpu information
                //Ignoring this too
            }
            _ => println!("Unknown gp1 command {:#X} parameter {}!", command.command(), command.parameter())
        }
    }

    pub fn get_vram(&self) -> &Vec<u16> {
        &self.vram
    }

    fn gp0_push(&mut self, val: u32) {
        self.gp0_buffer[self.gp0_buffer_address] = val;
        self.gp0_buffer_address += 1;
    }

    fn gp0_pop(&mut self) -> u32 {
        self.gp0_buffer_address -= 1;
        self.gp0_buffer[self.gp0_buffer_address]
    }

    fn gp0_clear(&mut self) {
        self.gp0_buffer_address = 0;
    }

    fn point_to_address(&self, x: u32, y: u32) -> u32 {
        ((1024) as u32 * y) + x
    }


    fn draw_horizontal_line(&mut self, x1: u32, x2: u32, y: u32, fill: u16) {
        for x in x1..=x2 {
            let address = self.point_to_address(x, y) as usize;
            self.vram[address] = fill;
        }
    }

    fn draw_solid_box(&mut self, x1: u32, y1: u32, x2: u32, y2: u32, fill: u16) {
        for y in y1..=y2 {
            self.draw_horizontal_line(x1, x2, y, fill);
        }
    }
}

//Helper trait + impl
trait Command {
    fn gp0_header(&self) -> u8;
    fn command(&self) -> u8;
    fn parameter(&self) -> u32;
}

impl Command for u32 {
    fn gp0_header(&self) -> u8 {
        ((self.clone() >> 29) & 0x7) as u8
    }

    fn command(&self) -> u8 {
        ((self.clone() >> 24) & 0xFF) as u8
    }

    fn parameter(&self) -> u32 {
        (self.clone() & 0x7FFFFF)
    }
}