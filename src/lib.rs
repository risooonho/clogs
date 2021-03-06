mod render;

use crate::render::Render;
use anyhow::{anyhow, Result};
use glsp::{GFn, GSend, Root, Runtime, Val};
use miniquad::{
    conf::{Conf, Loading},
    graphics::Context,
    EventHandler, UserData,
};
use smart_default::SmartDefault;

/// The main game object.
///
/// ## Example
///
/// ```rust
/// use clogs::Clog;
/// # fn main() {
/// let game = Clog::new("Title of the game")
///     .width(640)
///     .height(480);
///
/// // game.start();
/// # }
/// ```
#[derive(SmartDefault)]
pub struct Clog {
    /// The window title of the game.
    title: String,

    /// The GameLisp runtime.
    #[default(Runtime::new())]
    runtime: Runtime,

    /// The window width dimension.
    #[default = 800]
    width: i32,

    /// The window height dimension.
    #[default = 800]
    height: i32,

    /// How many MSAA samples are used for rendering the vector graphics.
    #[default = 8]
    sample_count: i32,

    /// SVGs to load.
    svgs: Vec<(String, String)>,

    /// Fonts to load.
    fonts: Vec<(String, String)>,
}

impl Clog {
    /// Setup a new game.
    pub fn new<T>(title: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            title: title.into(),
            runtime: Runtime::new(),
            ..Default::default()
        }
    }

    /// The main script of the game.
    ///
    /// Must be a GameLisp file containing the following functions:
    ///
    /// ```gamelisp
    /// engine:update
    /// engine:render
    /// ```
    pub fn main_script<S>(self, script: S) -> Result<Self>
    where
        S: AsRef<str> + GSend,
    {
        struct LoadingResult {
            has_update: bool,
            has_render: bool,
        };
        let result: LoadingResult = self
            .runtime
            .run(|| {
                // Execute the main lisp script
                glsp::eval_multi(&glsp::parse_all(script.as_ref(), None)?, None)?;

                // Check if the required functions are loaded
                Ok(LoadingResult {
                    has_update: Self::has_function("engine:update"),
                    has_render: Self::has_function("engine:render"),
                })
            })
            .ok_or(anyhow!("executing main script failed"))?;

        if !result.has_update {
            Err(anyhow!(
                "function 'engine:update' is missing from main script"
            ))
        } else if !result.has_render {
            Err(anyhow!(
                "function 'engine:render' is missing from main script"
            ))
        } else {
            Ok(self)
        }
    }

    /// Set the initial window width.
    pub fn width(mut self, width: i32) -> Self {
        self.width = width;

        self
    }

    /// Set the initial window height.
    pub fn height(mut self, height: i32) -> Self {
        self.height = height;

        self
    }

    /// Set how many MSAA samples are used for rendering the vector graphics.
    pub fn sample_count(mut self, sample_count: i32) -> Self {
        self.sample_count = sample_count;

        self
    }

    /// Add a SVG that will be uploaded to the GPU during the loading phase.
    ///
    /// The `reference_name` argument can be later used in scripts to create instances of the SVG
    /// mesh.
    /// The SVG format must not violate the [usvg limitations](https://github.com/RazrFalcon/resvg/tree/master/usvg#limitations).
    pub fn load_svg<R, S>(mut self, reference_name: R, svg_source: S) -> Self
    where
        S: Into<String>,
        R: Into<String>,
    {
        self.svgs.push((reference_name.into(), svg_source.into()));

        self
    }

    /// Start the game.
    pub fn start(self) {
        miniquad::start(
            Conf {
                window_title: self.title.clone(),
                window_width: self.width,
                window_height: self.height,
                loading: Loading::Embedded,
                sample_count: self.sample_count,
                ..Default::default()
            },
            |mut ctx| UserData::owning(ClogRun::new(&mut ctx, self.runtime), ctx),
        );
    }

    /// Check if a GLSP function is defined.
    fn has_function(function_name: &str) -> bool {
        match glsp::global(function_name) {
            Ok(Val::GFn(_)) => true,
            _ => false,
        }
    }
}

/// The actual game runtime.
struct ClogRun {
    /// The GameLisp runtime.
    runtime: Runtime,

    /// The render system.
    render: Render,
}

impl ClogRun {
    /// Create a new runtime.
    pub fn new(ctx: &mut Context, runtime: Runtime) -> Self {
        Self {
            runtime,
            render: Render::new(ctx),
        }
    }

    /// Run a GameLisp function.
    fn call(&self, function: &str) -> bool {
        struct RuntimeResult(bool);

        let result: RuntimeResult = self
            .runtime
            .run(|| {
                let update_func: Root<GFn> = match glsp::global(function) {
                    Ok(Val::GFn(update)) => update,
                    Ok(val) => {
                        eprintln!("invalid {} function: {}", function, val);

                        return Ok(RuntimeResult(false));
                    }
                    Err(err) => {
                        eprintln!("error finding {} function: {}", function, err);

                        return Ok(RuntimeResult(false));
                    }
                };
                let _: Val = glsp::call(&update_func, &())?;

                Ok(RuntimeResult(true))
            })
            .expect("Something unexpected went wrong with calling a GameLisp function");

        result.0
    }
}

impl EventHandler for ClogRun {
    fn update(&mut self, _: &mut Context) {
        self.call("engine:update");
    }

    fn draw(&mut self, _: &mut Context) {
        self.call("engine:render");
    }
}
