use nalgebra_glm as glm;
use sdl3::keyboard::KeyboardState;
use sdl3::keyboard::Scancode;

pub struct Camera {
    pub position: glm::Vec3,
    pub yaw: f32,   // degrees, horizontal
    pub pitch: f32, // degrees, vertical

    pub move_speed: f32,
    pub mouse_sensitivity: f32,
}

impl Camera {
    pub fn new(position: glm::Vec3, move_speed: f32) -> Self {
        Self {
            position,
            yaw: -90.0,
            pitch: 0.0,
            move_speed,
            mouse_sensitivity: 0.1,
        }
    }

    pub fn forward(&self) -> glm::Vec3 {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();
        glm::normalize(&glm::vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        ))
    }

    pub fn right(&self) -> glm::Vec3 {
        glm::normalize(&glm::cross(&self.forward(), &glm::vec3(0.0, 1.0, 0.0)))
    }

    pub fn process_mouse(&mut self, xrel: f32, yrel: f32, dt: f32) {
        self.yaw += xrel * self.mouse_sensitivity * dt * 100.0;
        self.pitch -= yrel * self.mouse_sensitivity * dt * 100.0;
        self.pitch = self.pitch.clamp(-89.0, 89.0);
    }

    pub fn process_keyboard(&mut self, kb: &KeyboardState, dt: f32) {
        let forward = self.forward();
        let right = self.right();
        let up = glm::vec3(0.0_f32, 1.0, 0.0);
        let speed = self.move_speed * dt;

        if kb.is_scancode_pressed(Scancode::W) {
            self.position += forward * speed;
        }
        if kb.is_scancode_pressed(Scancode::S) {
            self.position -= forward * speed;
        }
        if kb.is_scancode_pressed(Scancode::A) {
            self.position -= right * speed;
        }
        if kb.is_scancode_pressed(Scancode::D) {
            self.position += right * speed;
        }
        if kb.is_scancode_pressed(Scancode::Space) {
            self.position += up * speed;
        }
        if kb.is_scancode_pressed(Scancode::LShift) {
            self.position -= up * speed;
        }
    }

    pub fn view_matrix(&self) -> glm::Mat4 {
        glm::look_at(
            &self.position,
            &(self.position + self.forward()),
            &glm::vec3(0.0, 1.0, 0.0),
        )
    }
}
