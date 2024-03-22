use std::{fs::File, path::Path};

use liquid::{to_object, Parser, ParserBuilder};
use serde_json::Value;
use tracing::{debug, debug_span};

use crate::Result;

pub trait Renderer {
    fn render(&self, template: &str, dest: &Path, values: &Value) -> Result;
}

pub struct DefaultRenderer {
    parser: Parser,
}

impl DefaultRenderer {
    pub fn init() -> Result<Self> {
        Ok(Self {
            parser: ParserBuilder::with_stdlib().build()?,
        })
    }
}

impl Renderer for DefaultRenderer {
    fn render(&self, template: &str, dest: &Path, values: &Value) -> Result {
        let span = debug_span!("render", dest = %dest.display());
        let _enter = span.enter();
        debug!("parsing template");
        let template = self.parser.parse(template)?;
        debug!("converting values to liquid object");
        let obj = to_object(values)?;
        debug!("rendering template");
        let mut file = File::create(dest)?;
        template.render_to(&mut file, &obj)?;
        Ok(())
    }
}
