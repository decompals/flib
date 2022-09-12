
A symbol is defined in one file
Many other files may make reference to a symbol

We have maps
symbol -> defining file
symbol -> referencing files
file -> defined symbols
file -> referenced symbols

What is easy to calculate, and what is what we actually want to interpret?

Easy to calculate are the file -> symbols maps. symbol -> files requires searching all the other files

We want a graph of files, where `A -- s -> B` if `A` references a symbol `s` defined in `B`.


Best idea is probably to construct a dict
```py
symbol : { defining_file, [referencing-files], <other symbol info> }
```
This gives us arrows from R to d.

The hard part is getting the referencing file, i.e. linking.
