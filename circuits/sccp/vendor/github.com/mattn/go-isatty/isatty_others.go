//go:build (appengine || nacl || tinygo || haiku) && !windows
// +build appengine nacl tinygo haiku
// +build !windows

package isatty

// IsTerminal reports whether the file descriptor is a terminal.
// These platforms do not provide a terminal probe, so it returns false.
func IsTerminal(fd uintptr) bool {
	return false
}

// IsCygwinTerminal() return true if the file descriptor is a cygwin or msys2
// terminal. This is also always false on this environment.
func IsCygwinTerminal(fd uintptr) bool {
	return false
}
