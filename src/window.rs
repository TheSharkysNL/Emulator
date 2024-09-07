use glium::backend::glutin::SimpleWindowBuilder;
use glium::glutin::surface::WindowSurface;
use glium::{IndexBuffer, Program, Surface, VertexBuffer};
use glium::index::PrimitiveType;
use glium::winit::error::EventLoopError;
use glium::winit::event_loop::EventLoopBuilder;
use crate::computer::Computer;
use crate::cpu::CpuArchitecture;
use crate::display_handler::{AppHandler, Vertex};
use crate::instructions::{InstructionError, InstructionErrorKind, AWAITING_EVENT};
use crate::memory::{AllocatedRam, RamError};
use crate::operand::Register;

pub const VERTEX_SHADER_SRC: &str = r#"
                    #version 140

                    in vec2 position;
                    in uint color_number;
                    
                    out vec4 c;
                
                    void main() {
                        uint r = color_number & 0xFFu;
                        uint g = (color_number >> 8u) & 0xFFu;
                        uint b = (color_number >> 16u) & 0xFFu;
                        uint a = (color_number >> 24u) & 0xFFu;
                        c = vec4(float(r) / 255, float(g) / 255, float(b) / 255, float(a) / 255);
                        gl_Position = vec4(position, 0.0, 1.0);                    
                    }
                "#;

pub const FRAGMENT_SHADER_SRC: &str = r#"
                    #version 140
                
                    in vec4 c;
                    out vec4 color;
                
                    void main() {
                        color = c;
                    }
                "#;

pub(crate) fn vertex_buffer_from_memory(display: &glium::Display<WindowSurface>, ram: &AllocatedRam, size: (usize, usize)) -> Result<VertexBuffer<Vertex>, RamError> {
    let width_per_square = 2f32 / size.0 as f32;
    let height_per_square = 2f32 / size.1 as f32;

    let mut x = -1f32;
    let mut y = 1f32;
    let mut index = 0;

    let total_size = size.0 * size.1;
    let mut vertex_buffer = vec![Vertex::default();total_size * 4];

    while y > -1f32 + 0.0005 {
        while x <= 1f32 - 0.0005 {
            let color = ram.read_at::<u32>(index)?;

            let vertex_index = index as usize;
            vertex_buffer[vertex_index] = Vertex::new([x, y], color.to_le_bytes());
            vertex_buffer[vertex_index + 1] = Vertex::new([x + width_per_square, y], color.to_le_bytes());
            vertex_buffer[vertex_index + 2] = Vertex::new([x, y - height_per_square], color.to_le_bytes());
            vertex_buffer[vertex_index + 3] = Vertex::new([x + width_per_square, y - height_per_square], color.to_le_bytes());
            
            x += width_per_square;
            index += size_of::<u32>() as CpuArchitecture;
        }
        x = -1f32;
        y -= height_per_square;
    }

    let vertex_buffer = VertexBuffer::new(display, &vertex_buffer).unwrap();

    Ok(vertex_buffer)
}

fn index_buffer_from_size(display: &glium::Display<WindowSurface>, size: (usize, usize)) -> IndexBuffer<u32> {
    let total_size = size.0 * size.1;
    let mut index_buffer = vec![0; total_size * 6];

    let mut buf_index = 0;
    for index in (0..index_buffer.len()).step_by(6) {
        index_buffer[index] = buf_index + 1;
        index_buffer[index + 1] = buf_index;
        index_buffer[index + 2] = buf_index + 2;
        index_buffer[index + 3] = buf_index + 2;
        index_buffer[index + 4] = buf_index + 3;
        index_buffer[index + 5] = buf_index + 1;
        buf_index += 4;
    }

    IndexBuffer::immutable(display, PrimitiveType::TrianglesList, &index_buffer).unwrap()
}

pub struct Window { }

impl Window {
    pub fn run(canvas_size: (usize, usize), window_name: Option<&str>, computer: &mut Computer, alloc_base: Register) -> Result<(), InstructionError> {
        let result = EventLoopBuilder::default().build();
        let event_loop = match result {
            Ok(val) => val,
            Err(err) => return match err {
                EventLoopError::RecreationAttempt => Err(InstructionError::new(InstructionErrorKind::WindowAlreadyCreated)),
                _ => Err(InstructionError::with_message(InstructionErrorKind::Other, err.to_string())),
            },
        };
        let (window, display) = SimpleWindowBuilder::new().with_inner_size(1680, 1050).build(&event_loop);

        if let Some(window_name) = window_name {
            window.set_title(window_name);
        }
        window.set_resizable(false);

        let mem_size = canvas_size.0 * canvas_size.1 * size_of::<[u8;4]>();

        let mut alloc = computer.ram_mut().alloc(mem_size as CpuArchitecture)?;
        alloc.fill(0);

        let vertex_buffer = vertex_buffer_from_memory(&display, &alloc, canvas_size)?;
        let indices = index_buffer_from_size(&display, canvas_size);

        let program = Program::from_source(&display, VERTEX_SHADER_SRC, FRAGMENT_SHADER_SRC, None).unwrap();

        let mut frame = display.draw();
        frame.clear_color(1.0, 1.0, 1.0, 1.0);
        frame.draw(&vertex_buffer, &indices, &program, &glium::uniforms::EmptyUniforms,
                   &Default::default()).unwrap();
        frame.finish().unwrap();

        computer.cpu_mut().set_register(alloc_base, alloc.range().start)?; // same as above

        while !AWAITING_EVENT.get() {
            let result = computer.execute_next_instruction();
            match result {
                Ok(val) => if val { break; },
                Err(err) => return Err(InstructionError::with_message(InstructionErrorKind::Other, err.to_string())),
            };
        }

        let mut app_handler = AppHandler::new(computer, alloc, display, program, indices, canvas_size);
        event_loop.run_app(&mut app_handler).unwrap();


        if let Err(err) = app_handler.result() {
            Err(InstructionError::with_message(InstructionErrorKind::Other, err.to_string()))
        } else{
            Ok(())
        }
    }
}