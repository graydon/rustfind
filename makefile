test : rustfind
	@if [ ! $(RUST) ] ; then echo "set RUST to point to root of rust sourcetree" ; fi
	echo $(RUST)

	./main test_input.rs -L ~/gplsrc/rust/x86_64-unknown-linux-gnu/stage2/lib

rustfind: main.rs find_ast_node.rs astdump.rs text_formatting.rs 
	rustc main.rs


