#![no_std]

macro_rules! def_ld_sym {
    ($name:ident) => {
        pub fn $name() -> usize {
            extern "C" {
                static $name: usize;
            }
            unsafe { &$name as *const _ as usize }
        }
    };
}

def_ld_sym!(_stext);
def_ld_sym!(_etext);

def_ld_sym!(_srodata);
def_ld_sym!(_erodata);

def_ld_sym!(_spercpu);
def_ld_sym!(_epercpu);

def_ld_sym!(_sdata);
def_ld_sym!(_edata);

def_ld_sym!(_sbss);
def_ld_sym!(_ebss);

def_ld_sym!(_end);

def_ld_sym!(_sdev);
def_ld_sym!(_edev);

def_ld_sym!(_stest);
def_ld_sym!(_etest);
