use glium::{Display, implement_vertex, IndexBuffer, Program, Surface};
use glium::glutin::surface::WindowSurface;
use glium::uniforms::EmptyUniforms;
use glium::winit::application::ApplicationHandler;
use glium::winit::event::{KeyEvent, MouseButton, WindowEvent};
use glium::winit::event_loop::ActiveEventLoop;
use glium::winit::keyboard::{Key, NamedKey};
use glium::winit::window::WindowId;
use crate::computer::Computer;
use crate::cpu::CpuArchitecture;
use crate::instructions::{AWAITING_EVENT, REDRAW};
use crate::memory::AllocatedRam;
use crate::operand::Register;
use crate::error_creator;
use crate::computer::ComputerError;
use crate::window::vertex_buffer_from_memory;

error_creator!(
    AppError,
    AppErrorKind,
    ComputerError(ComputerError) => ""
);

pub(crate) struct AppHandler<'a> {
    computer: &'a mut Computer,
    error: Result<()>,
    
    memory: AllocatedRam,
    display: Display<WindowSurface>,
    program: Program,
    index_buffer: IndexBuffer<u32>,
    size: (usize, usize),
}

impl<'a> AppHandler<'a> {
    pub(crate) fn new(computer: &'a mut Computer, memory: AllocatedRam, display: Display<WindowSurface>,
                        program: Program, index_buffer: IndexBuffer<u32>, size: (usize, usize)) -> Self {
        Self {
            computer,
            error: Ok(()),
            memory,
            display,
            program,
            index_buffer,
            size
        }
    }
    
    pub(crate) fn result(self) -> Result<()> {
        self.error
    }

    fn redraw(&self) {
        let vertex_buffer = vertex_buffer_from_memory(&self.display, &self.memory, self.size).unwrap();

        let mut frame = self.display.draw();
        frame.clear_color(1.0, 1.0, 1.0, 1.0);
        frame.draw(&vertex_buffer, &self.index_buffer, &self.program,
                   &EmptyUniforms, &Default::default()).unwrap();
        frame.finish().unwrap();
    }
}

impl<'a> ApplicationHandler for AppHandler<'a> {
    fn resumed(&mut self, _: &ActiveEventLoop) {
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let event_num = match event {
            WindowEvent::CloseRequested => 0,
            WindowEvent::CursorMoved { position, .. } => {
                let x_register = Register::new(2, size_of::<CpuArchitecture>() as u8);
                let y_register = Register::new(3, size_of::<CpuArchitecture>() as u8);
                
                let dimensions = self.display.get_framebuffer_dimensions();
                let x = ((position.x / dimensions.0 as f64) * self.size.0 as f64) as CpuArchitecture;
                let y = ((position.y / dimensions.1 as f64) * self.size.1 as f64) as CpuArchitecture;
                let x = x.min((self.size.0 - 1) as CpuArchitecture);
                let y = y.min((self.size.1 - 1) as CpuArchitecture);
                
                self.computer.cpu_mut().set_register(x_register, x).unwrap(); // cpu should have 4 or more registers
                self.computer.cpu_mut().set_register(y_register, y).unwrap();
                1
            },
            WindowEvent::MouseInput { state, button, .. } => {
                let is_press_register = Register::new(2, size_of::<CpuArchitecture>() as u8);
                let button_num_register = Register::new(3, size_of::<CpuArchitecture>() as u8);
                
                let pressed = state.is_pressed() as CpuArchitecture;
                let button_num = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Forward => 3,
                    MouseButton::Back => 4,
                    MouseButton::Other(val) => val as CpuArchitecture + 4,
                };
                
                self.computer.cpu_mut().set_register(is_press_register, pressed).unwrap(); // cpu should have 4 or more registers
                self.computer.cpu_mut().set_register(button_num_register, button_num).unwrap();
                
                2
            },
            WindowEvent::KeyboardInput { event, .. } => {
                let button_register = Register::new(2, size_of::<CpuArchitecture>() as u8);
                let down_register = Register::new(3, size_of::<CpuArchitecture>() as u8);

                let KeyEvent { logical_key, .. } = event;
                let button = match logical_key {
                    Key::Character(c) => {
                        c.chars().next().unwrap()
                    },
                    Key::Named(named) => {
                        match named {
                            NamedKey::Enter => '\n',
                            _ => '\0',
                        }  
                    },
                    _ => '\0',
                };
                
                let down = event.state.is_pressed() as CpuArchitecture;
                
                self.computer.cpu_mut().set_register(button_register, button as CpuArchitecture).unwrap(); // cpu should have more than 4 registers
                self.computer.cpu_mut().set_register(down_register, down).unwrap();
                
                3
            },
            _ => CpuArchitecture::MAX,
        };
        
        let register = Register::new(1, size_of::<CpuArchitecture>() as u8);
        self.computer.cpu_mut().set_register(register, event_num).unwrap(); // cpu should have 4 or more registers
        
        while !AWAITING_EVENT.get() {
            let result = self.computer.execute_next_instruction();
            let exited = match result {
                Ok(val) => val,
                Err(err) => {
                    self.error = Err(AppError::new(AppErrorKind::ComputerError(err)));
                    event_loop.exit();
                    break;
                }
            };
            if exited {
                event_loop.exit();
                break;
            }
            if REDRAW.get() {
                self.redraw();
                REDRAW.set(false);
            }
        }
        AWAITING_EVENT.set(false);
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Vertex {
    position: [f32;2],
    color_number: u32,
}

impl Vertex {
    pub fn new(position: [f32;2], color:[u8;4]) -> Self {
        Self {
            position,
            color_number: u32::from_le_bytes(color),
        }
    }
}

implement_vertex!(Vertex, position, color_number);