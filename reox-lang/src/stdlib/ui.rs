// REOX Standard Library - UI Module
// Provides color manipulation and animation utilities

/// Create an RGB color value
/// rgb(255, 128, 64) -> Color
pub fn rgb(r: u8, g: u8, b: u8) -> crate::interpreter::Value {
    crate::interpreter::Value::Color { r, g, b, a: 255 }
}

/// Create an RGBA color value with alpha
/// rgba(255, 128, 64, 200) -> Color
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> crate::interpreter::Value {
    crate::interpreter::Value::Color { r, g, b, a }
}

/// Create a color from hex value
/// hex(0xFF8040) -> Color
pub fn hex(value: u32) -> crate::interpreter::Value {
    let r = ((value >> 16) & 0xFF) as u8;
    let g = ((value >> 8) & 0xFF) as u8;
    let b = (value & 0xFF) as u8;
    crate::interpreter::Value::Color { r, g, b, a: 255 }
}

/// Convert HSL to RGB color
/// h: 0-360, s: 0-100, l: 0-100
pub fn hsl(h: f64, s: f64, l: f64) -> crate::interpreter::Value {
    let s = s / 100.0;
    let l = l / 100.0;
    
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    
    let (r1, g1, b1) = match h as i32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    
    let r = ((r1 + m) * 255.0) as u8;
    let g = ((g1 + m) * 255.0) as u8;
    let b = ((b1 + m) * 255.0) as u8;
    
    crate::interpreter::Value::Color { r, g, b, a: 255 }
}

// ============== Animation Easing Functions ==============

/// Linear easing (no easing)
pub fn ease_linear(t: f64) -> f64 {
    t.clamp(0.0, 1.0)
}

/// Ease-in (accelerate)
pub fn ease_in(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// Ease-out (decelerate)
pub fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Ease-in-out (smooth S-curve)
pub fn ease_in_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Cubic ease-in
pub fn ease_in_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

/// Cubic ease-out
pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Bounce ease-out
pub fn ease_out_bounce(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let n1 = 7.5625;
    let d1 = 2.75;
    
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

// ============== Interpolation Functions ==============

/// Linear interpolation between two values
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    a + (b - a) * t
}

/// Interpolate between two colors
pub fn color_lerp(c1: (u8, u8, u8, u8), c2: (u8, u8, u8, u8), t: f64) -> crate::interpreter::Value {
    let t = t.clamp(0.0, 1.0);
    let r = lerp(c1.0 as f64, c2.0 as f64, t) as u8;
    let g = lerp(c1.1 as f64, c2.1 as f64, t) as u8;
    let b = lerp(c1.2 as f64, c2.2 as f64, t) as u8;
    let a = lerp(c1.3 as f64, c2.3 as f64, t) as u8;
    crate::interpreter::Value::Color { r, g, b, a }
}

// ============== View Modifier System ==============

/// ViewModifier for SwiftUI-like chainable styling
#[derive(Debug, Clone, Default)]
pub struct ViewModifier {
    pub background_color: Option<(u8, u8, u8, u8)>,
    pub background_image: Option<String>,
    pub padding: Option<Insets>,
    pub corner_radius: Option<f64>,
    pub shadow: Option<Shadow>,
    pub frame: Option<Frame>,
    pub opacity: Option<f64>,
    pub border: Option<Border>,
}

#[derive(Debug, Clone, Default)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone)]
pub struct Shadow {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub color: (u8, u8, u8, u8),
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Border {
    pub width: f64,
    pub color: (u8, u8, u8, u8),
}

impl ViewModifier {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set background color
    pub fn background_color(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.background_color = Some((r, g, b, a));
        self
    }
    
    /// Set background image (checks if file exists)
    pub fn background_image(mut self, path: &str) -> Self {
        if std::path::Path::new(path).exists() {
            self.background_image = Some(path.to_string());
        }
        self
    }
    
    /// Set padding (all sides)
    pub fn padding_all(mut self, value: f64) -> Self {
        self.padding = Some(Insets {
            top: value, right: value, bottom: value, left: value,
        });
        self
    }
    
    /// Set padding (individual sides)
    pub fn padding(mut self, top: f64, right: f64, bottom: f64, left: f64) -> Self {
        self.padding = Some(Insets { top, right, bottom, left });
        self
    }
    
    /// Set corner radius
    pub fn corner_radius(mut self, radius: f64) -> Self {
        self.corner_radius = Some(radius);
        self
    }
    
    /// Add shadow
    pub fn shadow(mut self, x: f64, y: f64, blur: f64, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.shadow = Some(Shadow { x, y, blur, color: (r, g, b, a) });
        self
    }
    
    /// Set frame size
    pub fn frame(mut self, width: f64, height: f64) -> Self {
        self.frame = Some(Frame {
            width: Some(width),
            height: Some(height),
            min_width: None, min_height: None,
            max_width: None, max_height: None,
        });
        self
    }
    
    /// Set opacity (0.0 - 1.0)
    pub fn opacity(mut self, alpha: f64) -> Self {
        self.opacity = Some(alpha.clamp(0.0, 1.0));
        self
    }
    
    /// Add border
    pub fn border(mut self, width: f64, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.border = Some(Border { width, color: (r, g, b, a) });
        self
    }
    
    /// Generate C code for this modifier (used by codegen)
    pub fn to_c_code(&self, view_var: &str) -> String {
        let mut code = String::new();
        
        if let Some((r, g, b, a)) = self.background_color {
            code.push_str(&format!(
                "{view_var}->box.background = (rx_color){{ {r}, {g}, {b}, {a} }};\n"
            ));
        }
        
        if let Some(ref path) = self.background_image {
            code.push_str(&format!(
                "{view_var}->box.background_image = \"{path}\";\n"
            ));
        }
        
        if let Some(ref p) = self.padding {
            code.push_str(&format!(
                "{view_var}->box.padding = insets({}, {}, {}, {});\n",
                p.top, p.right, p.bottom, p.left
            ));
        }
        
        if let Some(r) = self.corner_radius {
            code.push_str(&format!(
                "{view_var}->box.corner_radius = corners_all({r});\n"
            ));
        }
        
        if let Some(ref s) = self.shadow {
            code.push_str(&format!(
                "{view_var}->box.shadow = shadow({}, {}, {}, (rx_color){{ {}, {}, {}, {} }});\n",
                s.x, s.y, s.blur, s.color.0, s.color.1, s.color.2, s.color.3
            ));
        }
        
        if let Some(ref f) = self.frame {
            if let Some(w) = f.width {
                code.push_str(&format!("{view_var}->box.width = {w};\n"));
            }
            if let Some(h) = f.height {
                code.push_str(&format!("{view_var}->box.height = {h};\n"));
            }
        }
        
        if let Some(o) = self.opacity {
            code.push_str(&format!("{view_var}->opacity = {o};\n"));
        }
        
        if let Some(ref b) = self.border {
            code.push_str(&format!(
                "{view_var}->box.border_width = {};\n{view_var}->box.border_color = (rx_color){{ {}, {}, {}, {} }};\n",
                b.width, b.color.0, b.color.1, b.color.2, b.color.3
            ));
        }
        
        code
    }
}

// ============== Predefined Theme Colors ==============

pub fn color_primary() -> (u8, u8, u8, u8) { (0, 122, 255, 255) }
pub fn color_secondary() -> (u8, u8, u8, u8) { (88, 86, 214, 255) }
pub fn color_success() -> (u8, u8, u8, u8) { (52, 199, 89, 255) }
pub fn color_warning() -> (u8, u8, u8, u8) { (255, 149, 0, 255) }
pub fn color_danger() -> (u8, u8, u8, u8) { (255, 59, 48, 255) }
pub fn color_background() -> (u8, u8, u8, u8) { (28, 28, 30, 255) }
pub fn color_surface() -> (u8, u8, u8, u8) { (44, 44, 46, 255) }
pub fn color_text() -> (u8, u8, u8, u8) { (255, 255, 255, 255) }
pub fn color_text_dim() -> (u8, u8, u8, u8) { (142, 142, 147, 255) }

// ============== Container Builder Helpers ==============

/// Represents a container layout for code generation
#[derive(Debug, Clone)]
pub enum ContainerType {
    VStack { gap: f64 },
    HStack { gap: f64 },
    ZStack,
    Scroll { direction: ScrollDirection },
}

#[derive(Debug, Clone)]
pub enum ScrollDirection {
    Vertical,
    Horizontal,
    Both,
}

/// Container builder for SwiftUI-like syntax
#[derive(Debug, Clone)]
pub struct ContainerBuilder {
    pub container_type: ContainerType,
    pub children: Vec<String>,  // C code for children
    pub modifiers: ViewModifier,
}

impl ContainerBuilder {
    pub fn vstack(gap: f64) -> Self {
        Self {
            container_type: ContainerType::VStack { gap },
            children: Vec::new(),
            modifiers: ViewModifier::new(),
        }
    }
    
    pub fn hstack(gap: f64) -> Self {
        Self {
            container_type: ContainerType::HStack { gap },
            children: Vec::new(),
            modifiers: ViewModifier::new(),
        }
    }
    
    pub fn zstack() -> Self {
        Self {
            container_type: ContainerType::ZStack,
            children: Vec::new(),
            modifiers: ViewModifier::new(),
        }
    }
    
    pub fn add_child(&mut self, child_code: String) {
        self.children.push(child_code);
    }
    
    pub fn with_modifiers(mut self, modifiers: ViewModifier) -> Self {
        self.modifiers = modifiers;
        self
    }
    
    /// Generate C code for this container
    pub fn to_c_code(&self, var_name: &str) -> String {
        let mut code = String::new();
        
        // Create container
        match &self.container_type {
            ContainerType::VStack { gap } => {
                code.push_str(&format!("rx_view* {var_name} = reox_vstack({gap});\n"));
            }
            ContainerType::HStack { gap } => {
                code.push_str(&format!("rx_view* {var_name} = reox_hstack({gap});\n"));
            }
            ContainerType::ZStack => {
                code.push_str(&format!("rx_view* {var_name} = reox_zstack();\n"));
            }
            ContainerType::Scroll { direction } => {
                let dir = match direction {
                    ScrollDirection::Vertical => "SCROLL_VERTICAL",
                    ScrollDirection::Horizontal => "SCROLL_HORIZONTAL", 
                    ScrollDirection::Both => "SCROLL_BOTH",
                };
                code.push_str(&format!("rx_view* {var_name} = reox_scroll({dir});\n"));
            }
        }
        
        // Apply modifiers
        code.push_str(&self.modifiers.to_c_code(var_name));
        
        // Add children
        for child in &self.children {
            code.push_str(&format!("reox_container_add({var_name}, {child});\n"));
        }
        
        code
    }
}

// ============== Widget Builders ==============

/// Button widget with hover states and click handler
#[derive(Debug, Clone)]
pub struct ButtonBuilder {
    pub label: String,
    pub normal_color: (u8, u8, u8, u8),
    pub hover_color: (u8, u8, u8, u8),
    pub pressed_color: (u8, u8, u8, u8),
    pub disabled_color: (u8, u8, u8, u8),
    pub text_color: (u8, u8, u8, u8),
    pub font_size: f64,
    pub corner_radius: f64,
    pub padding: f64,
    pub enabled: bool,
    pub on_click: Option<String>,  // C function name
}

impl Default for ButtonBuilder {
    fn default() -> Self {
        Self {
            label: String::new(),
            normal_color: (0, 122, 255, 255),    // Primary blue
            hover_color: (30, 144, 255, 255),    // Lighter blue
            pressed_color: (0, 100, 220, 255),   // Darker blue
            disabled_color: (88, 88, 92, 255),   // Gray
            text_color: (255, 255, 255, 255),    // White
            font_size: 16.0,
            corner_radius: 8.0,
            padding: 12.0,
            enabled: true,
            on_click: None,
        }
    }
}

impl ButtonBuilder {
    pub fn new(label: &str) -> Self {
        Self { label: label.to_string(), ..Default::default() }
    }
    
    pub fn primary(label: &str) -> Self {
        Self::new(label)
    }
    
    pub fn secondary(label: &str) -> Self {
        Self {
            label: label.to_string(),
            normal_color: (88, 86, 214, 255),
            hover_color: (108, 106, 234, 255),
            pressed_color: (68, 66, 194, 255),
            ..Default::default()
        }
    }
    
    pub fn danger(label: &str) -> Self {
        Self {
            label: label.to_string(),
            normal_color: (255, 59, 48, 255),
            hover_color: (255, 89, 78, 255),
            pressed_color: (220, 50, 40, 255),
            ..Default::default()
        }
    }
    
    pub fn ghost(label: &str) -> Self {
        Self {
            label: label.to_string(),
            normal_color: (0, 0, 0, 0),
            hover_color: (255, 255, 255, 20),
            pressed_color: (255, 255, 255, 40),
            text_color: (0, 122, 255, 255),
            ..Default::default()
        }
    }
    
    pub fn on_click(mut self, handler: &str) -> Self {
        self.on_click = Some(handler.to_string());
        self
    }
    
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
    
    pub fn font_size(mut self, size: f64) -> Self {
        self.font_size = size;
        self
    }
    
    pub fn corner_radius(mut self, radius: f64) -> Self {
        self.corner_radius = radius;
        self
    }
    
    pub fn to_c_code(&self, var_name: &str) -> String {
        let mut code = format!(
            "rx_button_view* {var_name} = button_view_new(\"{}\");\n",
            self.label
        );
        code.push_str(&format!(
            "{var_name}->normal_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.normal_color.0, self.normal_color.1, self.normal_color.2, self.normal_color.3
        ));
        code.push_str(&format!(
            "{var_name}->hover_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.hover_color.0, self.hover_color.1, self.hover_color.2, self.hover_color.3
        ));
        code.push_str(&format!(
            "{var_name}->pressed_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.pressed_color.0, self.pressed_color.1, self.pressed_color.2, self.pressed_color.3
        ));
        code.push_str(&format!(
            "{var_name}->text_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.text_color.0, self.text_color.1, self.text_color.2, self.text_color.3
        ));
        code.push_str(&format!("{var_name}->font_size = {};\n", self.font_size));
        code.push_str(&format!("{var_name}->corner_radius = {};\n", self.corner_radius));
        code.push_str(&format!("{var_name}->enabled = {};\n", if self.enabled { "true" } else { "false" }));
        
        if let Some(ref handler) = self.on_click {
            code.push_str(&format!("{var_name}->on_click = {handler};\n"));
        }
        
        code
    }
}

/// Text/Label widget with styling
#[derive(Debug, Clone)]
pub struct TextBuilder {
    pub text: String,
    pub color: (u8, u8, u8, u8),
    pub font_size: f64,
    pub font_weight: i32,
    pub alignment: TextAlign,
    pub max_lines: Option<i32>,
    pub line_height: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl Default for TextBuilder {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: (255, 255, 255, 255),
            font_size: 16.0,
            font_weight: 400,
            alignment: TextAlign::Left,
            max_lines: None,
            line_height: 1.4,
        }
    }
}

impl TextBuilder {
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string(), ..Default::default() }
    }
    
    pub fn title(text: &str) -> Self {
        Self {
            text: text.to_string(),
            font_size: 28.0,
            font_weight: 700,
            ..Default::default()
        }
    }
    
    pub fn subtitle(text: &str) -> Self {
        Self {
            text: text.to_string(),
            font_size: 20.0,
            font_weight: 600,
            color: (200, 200, 200, 255),
            ..Default::default()
        }
    }
    
    pub fn caption(text: &str) -> Self {
        Self {
            text: text.to_string(),
            font_size: 12.0,
            color: (142, 142, 147, 255),
            ..Default::default()
        }
    }
    
    pub fn color(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.color = (r, g, b, a);
        self
    }
    
    pub fn font_size(mut self, size: f64) -> Self {
        self.font_size = size;
        self
    }
    
    pub fn bold(mut self) -> Self {
        self.font_weight = 700;
        self
    }
    
    pub fn center(mut self) -> Self {
        self.alignment = TextAlign::Center;
        self
    }
    
    pub fn to_c_code(&self, var_name: &str) -> String {
        let mut code = format!(
            "rx_text_view* {var_name} = text_view_new(\"{}\");\n",
            self.text.replace('"', "\\\"")
        );
        code.push_str(&format!("text_view_set_font_size({var_name}, {});\n", self.font_size));
        code.push_str(&format!("{var_name}->font_weight = {};\n", self.font_weight));
        code.push_str(&format!(
            "{var_name}->color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.color.0, self.color.1, self.color.2, self.color.3
        ));
        let align = match self.alignment {
            TextAlign::Left => "TEXT_ALIGN_LEFT",
            TextAlign::Center => "TEXT_ALIGN_CENTER",
            TextAlign::Right => "TEXT_ALIGN_RIGHT",
        };
        code.push_str(&format!("{var_name}->alignment = {align};\n"));
        code
    }
}

/// Input field widget
#[derive(Debug, Clone)]
pub struct InputBuilder {
    pub placeholder: String,
    pub value: String,
    pub is_password: bool,
    pub is_multiline: bool,
    pub background_color: (u8, u8, u8, u8),
    pub text_color: (u8, u8, u8, u8),
    pub placeholder_color: (u8, u8, u8, u8),
    pub border_color: (u8, u8, u8, u8),
    pub focus_border_color: (u8, u8, u8, u8),
    pub corner_radius: f64,
    pub on_change: Option<String>,
    pub on_submit: Option<String>,
}

impl Default for InputBuilder {
    fn default() -> Self {
        Self {
            placeholder: String::new(),
            value: String::new(),
            is_password: false,
            is_multiline: false,
            background_color: (44, 44, 46, 255),
            text_color: (255, 255, 255, 255),
            placeholder_color: (142, 142, 147, 255),
            border_color: (72, 72, 74, 255),
            focus_border_color: (0, 122, 255, 255),
            corner_radius: 8.0,
            on_change: None,
            on_submit: None,
        }
    }
}

impl InputBuilder {
    pub fn new(placeholder: &str) -> Self {
        Self { placeholder: placeholder.to_string(), ..Default::default() }
    }
    
    pub fn password(placeholder: &str) -> Self {
        Self {
            placeholder: placeholder.to_string(),
            is_password: true,
            ..Default::default()
        }
    }
    
    pub fn multiline(placeholder: &str) -> Self {
        Self {
            placeholder: placeholder.to_string(),
            is_multiline: true,
            ..Default::default()
        }
    }
    
    pub fn value(mut self, val: &str) -> Self {
        self.value = val.to_string();
        self
    }
    
    pub fn on_change(mut self, handler: &str) -> Self {
        self.on_change = Some(handler.to_string());
        self
    }
    
    pub fn on_submit(mut self, handler: &str) -> Self {
        self.on_submit = Some(handler.to_string());
        self
    }
    
    pub fn to_c_code(&self, var_name: &str) -> String {
        let mut code = format!(
            "rx_input_view* {var_name} = input_view_new(\"{}\");\n",
            self.placeholder
        );
        if !self.value.is_empty() {
            code.push_str(&format!("input_view_set_text({var_name}, \"{}\");\n", self.value));
        }
        code.push_str(&format!("{var_name}->is_password = {};\n", if self.is_password { "true" } else { "false" }));
        code.push_str(&format!("{var_name}->is_multiline = {};\n", if self.is_multiline { "true" } else { "false" }));
        code.push_str(&format!(
            "{var_name}->background_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.background_color.0, self.background_color.1, self.background_color.2, self.background_color.3
        ));
        code.push_str(&format!(
            "{var_name}->text_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.text_color.0, self.text_color.1, self.text_color.2, self.text_color.3
        ));
        code.push_str(&format!("{var_name}->corner_radius = {};\n", self.corner_radius));
        
        if let Some(ref handler) = self.on_change {
            code.push_str(&format!("{var_name}->on_change = {handler};\n"));
        }
        if let Some(ref handler) = self.on_submit {
            code.push_str(&format!("{var_name}->on_submit = {handler};\n"));
        }
        
        code
    }
}

/// Checkbox/Toggle widget
#[derive(Debug, Clone)]
pub struct CheckboxBuilder {
    pub label: String,
    pub checked: bool,
    pub on_color: (u8, u8, u8, u8),
    pub off_color: (u8, u8, u8, u8),
    pub on_change: Option<String>,
}

impl Default for CheckboxBuilder {
    fn default() -> Self {
        Self {
            label: String::new(),
            checked: false,
            on_color: (52, 199, 89, 255),    // Green
            off_color: (72, 72, 74, 255),    // Gray
            on_change: None,
        }
    }
}

impl CheckboxBuilder {
    pub fn new(label: &str) -> Self {
        Self { label: label.to_string(), ..Default::default() }
    }
    
    pub fn checked(mut self) -> Self {
        self.checked = true;
        self
    }
    
    pub fn on_change(mut self, handler: &str) -> Self {
        self.on_change = Some(handler.to_string());
        self
    }
    
    pub fn to_c_code(&self, var_name: &str) -> String {
        let mut code = format!(
            "rx_checkbox_view* {var_name} = checkbox_view_new(\"{}\");\n",
            self.label
        );
        code.push_str(&format!("{var_name}->checked = {};\n", if self.checked { "true" } else { "false" }));
        code.push_str(&format!(
            "{var_name}->on_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.on_color.0, self.on_color.1, self.on_color.2, self.on_color.3
        ));
        if let Some(ref handler) = self.on_change {
            code.push_str(&format!("{var_name}->on_change = {handler};\n"));
        }
        code
    }
}

/// Slider widget
#[derive(Debug, Clone)]
pub struct SliderBuilder {
    pub min: f64,
    pub max: f64,
    pub value: f64,
    pub track_color: (u8, u8, u8, u8),
    pub active_color: (u8, u8, u8, u8),
    pub thumb_color: (u8, u8, u8, u8),
    pub on_change: Option<String>,
}

impl Default for SliderBuilder {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            value: 50.0,
            track_color: (72, 72, 74, 255),
            active_color: (0, 122, 255, 255),
            thumb_color: (255, 255, 255, 255),
            on_change: None,
        }
    }
}

impl SliderBuilder {
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max, value: (min + max) / 2.0, ..Default::default() }
    }
    
    pub fn value(mut self, val: f64) -> Self {
        self.value = val.clamp(self.min, self.max);
        self
    }
    
    pub fn on_change(mut self, handler: &str) -> Self {
        self.on_change = Some(handler.to_string());
        self
    }
    
    pub fn to_c_code(&self, var_name: &str) -> String {
        let mut code = format!(
            "rx_slider_view* {var_name} = slider_view_new({}, {});\n",
            self.min, self.max
        );
        code.push_str(&format!("slider_view_set_value({var_name}, {});\n", self.value));
        code.push_str(&format!(
            "{var_name}->active_color = (rx_color){{ {}, {}, {}, {} }};\n",
            self.active_color.0, self.active_color.1, self.active_color.2, self.active_color.3
        ));
        if let Some(ref handler) = self.on_change {
            code.push_str(&format!("{var_name}->on_change = {handler};\n"));
        }
        code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rgb() {
        if let crate::interpreter::Value::Color { r, g, b, a } = rgb(255, 128, 64) {
            assert_eq!(r, 255);
            assert_eq!(g, 128);
            assert_eq!(b, 64);
            assert_eq!(a, 255);
        } else {
            panic!("Expected Color");
        }
    }
    
    #[test]
    fn test_hex() {
        if let crate::interpreter::Value::Color { r, g, b, .. } = hex(0xFF8040) {
            assert_eq!(r, 255);
            assert_eq!(g, 128);
            assert_eq!(b, 64);
        } else {
            panic!("Expected Color");
        }
    }
    
    #[test]
    fn test_ease_linear() {
        assert!((ease_linear(0.0) - 0.0).abs() < 0.001);
        assert!((ease_linear(0.5) - 0.5).abs() < 0.001);
        assert!((ease_linear(1.0) - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_ease_in() {
        assert!((ease_in(0.0) - 0.0).abs() < 0.001);
        assert!((ease_in(0.5) - 0.25).abs() < 0.001);
        assert!((ease_in(1.0) - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 0.001);
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 0.001);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 0.001);
    }
}
