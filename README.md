# preproc

Simple and configurable SIMD pre-processor, with a throughput of up to 3 GiB/s

# Quirks and Other Notes

- Supports only UTF-8
- SSE2 required, no NEON support for the time been
- Whitespaces are considered to be `' ' (0x20)` and `'\t' (0x09)`
- Multiline comments aren't supported, (they work in some situations, but is best to avoid them)
- Unary operators can be placed on left e.g. `!a == a!` and `!(a && b) == (a && b)!`

# Samples

```c
//#if MY_MACRO // this directive is commented out
#if MY_OTHER_MACRO || MY_MACRO // this directive is active, single line comments are fine
// your code here
            #endif // doesn't care about white spaces as long the '#' is the frist char in the line
```

```c
// invalid multiline comments
/*#if MY_MACRO // won't be treated as a directive and won't be able to output the right code
// your code here
#endif*/

// valid multiline comments styles
/*
#if MY_MACRO
// your code here
#endif*/

/*
#if MY_MACRO
// your code here
#endif
*/
```

# Usage