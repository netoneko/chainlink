//! Output abstraction for std/no_std compatibility
//!
//! This module provides a trait-based abstraction for output operations,
//! allowing the same command code to work in both std and no_std environments.

use core::fmt::Arguments;

/// Trait for output operations
/// 
/// Implement this trait to provide custom output handling in no_std environments.
pub trait Output {
    /// Print a string without newline
    fn print(&self, s: &str);
    
    /// Print a string with newline
    fn println(&self, s: &str);
    
    /// Print to stderr without newline
    fn eprint(&self, s: &str);
    
    /// Print to stderr with newline
    fn eprintln(&self, s: &str);
    
    /// Print formatted output without newline
    fn print_fmt(&self, args: Arguments<'_>) {
        #[cfg(feature = "std")]
        self.print(&std::fmt::format(args));
        #[cfg(not(feature = "std"))]
        self.print(&alloc::fmt::format(args));
    }
    
    /// Print formatted output with newline
    fn println_fmt(&self, args: Arguments<'_>) {
        #[cfg(feature = "std")]
        self.println(&std::fmt::format(args));
        #[cfg(not(feature = "std"))]
        self.println(&alloc::fmt::format(args));
    }
    
    /// Print formatted output to stderr without newline
    fn eprint_fmt(&self, args: Arguments<'_>) {
        #[cfg(feature = "std")]
        self.eprint(&std::fmt::format(args));
        #[cfg(not(feature = "std"))]
        self.eprint(&alloc::fmt::format(args));
    }
    
    /// Print formatted output to stderr with newline
    fn eprintln_fmt(&self, args: Arguments<'_>) {
        #[cfg(feature = "std")]
        self.eprintln(&std::fmt::format(args));
        #[cfg(not(feature = "std"))]
        self.eprintln(&alloc::fmt::format(args));
    }
}

/// Standard output implementation (available with std feature)
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, Default)]
pub struct StdOutput;

#[cfg(feature = "std")]
impl Output for StdOutput {
    fn print(&self, s: &str) { 
        print!("{}", s); 
    }
    
    fn println(&self, s: &str) { 
        println!("{}", s); 
    }
    
    fn eprint(&self, s: &str) { 
        eprint!("{}", s); 
    }
    
    fn eprintln(&self, s: &str) { 
        eprintln!("{}", s); 
    }
}

/// Convenience macro for printing without newline
/// 
/// Usage: `out_print!(out, "Hello, {}!", name)`
#[macro_export]
macro_rules! out_print {
    ($out:expr, $($arg:tt)*) => {
        $out.print_fmt(format_args!($($arg)*))
    };
}

/// Convenience macro for printing with newline
/// 
/// Usage: `out_println!(out, "Hello, {}!", name)` or `out_println!(out)` for empty line
#[macro_export]
macro_rules! out_println {
    ($out:expr) => { $out.println("") };
    ($out:expr, $($arg:tt)*) => {
        $out.println_fmt(format_args!($($arg)*))
    };
}

/// Convenience macro for printing to stderr without newline
#[macro_export]
macro_rules! out_eprint {
    ($out:expr, $($arg:tt)*) => {
        $out.eprint_fmt(format_args!($($arg)*))
    };
}

/// Convenience macro for printing to stderr with newline
#[macro_export]
macro_rules! out_eprintln {
    ($out:expr) => { $out.eprintln("") };
    ($out:expr, $($arg:tt)*) => {
        $out.eprintln_fmt(format_args!($($arg)*))
    };
}
