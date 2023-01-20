use crate::structs::Class;

pub struct Frame<'a> {
    class: &'a Class,
    ip: usize,
    code: &'a [u8],
    locals: Vec<()>,
    stack: Vec<()>,
}

impl Frame<'_> {
    fn execute(&mut self) {
        loop {
            let op = self.code[self.ip];
            match op {
                26 => {
                    self.stack.push(self.locals[0]);
                }
                27 => {
                    self.stack.push(self.locals[1]);
                }
                96 => {
                    let a = self.stack.pop().unwrap();
                    let b = self.stack.pop().unwrap();
                }
                _ => {
                    // skip unknown instructions
                }
            }

            self.ip += 1;
        }
    }
}
