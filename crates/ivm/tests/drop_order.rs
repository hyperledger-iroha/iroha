//! Verifies Rust 1.91 pattern-binding drop order matches expectations so Kotodama/IVM code
//! does not rely on pre-1.91 behaviour.

#[derive(Debug)]
struct DropLog {
    name: &'static str,
    log: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
}

impl Drop for DropLog {
    fn drop(&mut self) {
        self.log.borrow_mut().push(self.name);
    }
}

#[test]
fn destructuring_binds_drop_in_reverse_declaration_order() {
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    {
        let (a, b) = (
            DropLog {
                name: "a",
                log: std::rc::Rc::clone(&log),
            },
            DropLog {
                name: "b",
                log: std::rc::Rc::clone(&log),
            },
        );
        // use bindings so they are not optimized out
        assert_eq!(a.name, "a");
        assert_eq!(b.name, "b");
    }

    assert_eq!(&*log.borrow(), &["b", "a"]);
}
