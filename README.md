# MSB - Maybe simple build (system)
A build system made purely for educational purposes
Supports incremental builds.

## Examples
 - See scuffed examples in examples/


## Format
```msb
target <Name> |outputs(...)|[files(...) targets(...)] {
    <Shell commands>
}
<More targets>
```
Name as a alphanumeric identifier
Identifiers in outputs, files can be any valid file path


## Really basic example
```msb
target main outputs(main) [files(main.c), targets()] {
    gcc main.c -o main
}
```
The outputs spec. can be ommited

## TODO
 - Macros
