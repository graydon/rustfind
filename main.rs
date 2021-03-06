
extern mod syntax;
extern mod rustc;
extern mod extra;

use rustc::{front, metadata, driver, middle};
use rustc::middle::*;

use syntax::parse;
use syntax::ast;
use syntax::ast_map;
use syntax::visit;
use syntax::visit::*;
use syntax::visit::{Visitor, fn_kind};
use find_ast_node::*;
use text_formatting::*;
use syntax::diagnostic;
use syntax::codemap::BytePos;

use syntax::abi::AbiSet;
use syntax::ast;
use syntax::codemap;


use std::os;
use std::local_data;
use extra::json::ToJson;

mod text_formatting;
mod find_ast_node;
mod ioutil;

pub static ctxtkey: local_data::Key<@DocContext> = &local_data::Key;


pub macro_rules! tlogi{ 
	($($a:expr),*)=>(println((file!()+":"+line!().to_str()+": " $(+$a.to_str())*) ))
}
pub macro_rules! logi{ 
	($($a:expr),*)=>(println(""$(+$a.to_str())*) )
}
//macro_rules! dump{ ($a:expr)=>(logi!(fmt!("%s=%?",stringify!($a),$a).indent(2,160));)}
macro_rules! dump{ ($($a:expr),*)=>
	(	{	let mut txt=~""; 
			$( { txt=txt.append(
				 fmt!("%s=%?",stringify!($a),$a)+",") 
				}
			);*; 
			logi!(txt); 
		}
	)
}


/// tags: crate,ast,parse resolve
/// Parses, resolves the given crate
fn get_ast_and_resolve(cpath: &Path, libs: ~[Path]) -> DocContext {
	


    let parsesess = parse::new_parse_sess(None);
    let sessopts = @driver::session::options {
        binary: @"rustdoc",
        maybe_sysroot: Some(@std::os::self_exe_path().get().pop()),
        addl_lib_search_paths: @mut libs,
        .. copy (*rustc::driver::session::basic_options())
    };

    let diagnostic_handler = syntax::diagnostic::mk_handler(None);
    let span_diagnostic_handler =
        syntax::diagnostic::mk_span_handler(diagnostic_handler, parsesess.cm);

    let mut sess = driver::driver::build_session_(sessopts, parsesess.cm,
                                                  syntax::diagnostic::emit,
                                                  span_diagnostic_handler);

    let (crate, tycx) = driver::driver::compile_upto(sess, sessopts.cfg.clone(),
                                                     &driver::driver::file_input(cpath.clone()),
                                                     driver::driver::cu_no_trans, None);
                                                     
	let c=crate.unwrap();
	let t=tycx.unwrap();
    DocContext { crate: c, tycx: t, sess: sess }
}


fn main() {
    use extra::getopts::*;
    use std::hashmap::HashMap;

    let args = os::args();

    let opts = ~[
        optmulti("L")
    ];

    let matches = getopts(args.tail(), opts).get();
    let libs = opt_strs(&matches, "L").map(|s| Path(*s));
	dump!(args,matches);
	dump!(libs);
    let dc = @get_ast_and_resolve(&Path(matches.free[0]), libs);
    local_data::set(ctxtkey, dc);

	debug_test(dc,matches.free[0]);
}

fn option_to_str<T:ToStr>(opt:&Option<T>)->~str {
	match *opt { Some(ref s)=>~"("+s.to_str()+~")",None=>~"(None)" }
}

trait MyToStr {  fn my_to_str(&self)->~str; }
impl MyToStr for codemap::span {
	fn my_to_str(&self)->~str { ~"("+self.lo.to_str()+~".."+self.hi.to_str() }
}

fn debug_test(dc:&DocContext,filename:~str) {

	// TODO: parse commandline source locations,convert to codemap locations
	//dump!(ctxt.tycx);

	logi!("loading",filename);
	let source_text = ioutil::fileLoad(filename);

	logi!("==== dump def table.===")
	dump_ctxt_def_map(dc);

	logi!("==== Get table of node-spans...===")
	let node_spans=build_node_spans_table(dc.crate);
	dump_node_spans_table(node_spans);

	logi!("==== Test node search by location...===")
 
	let mut source_pos=15 as uint;
	while source_pos<350 {
		// get the AST node under 'pos', and dump info
		let pos= text_offset_to_line_pos(source_text,source_pos);
		match (pos) {
			None=>logi!("position out of range"),
			Some((line,ofs))=> {
				logi!(~"\n==========Find AST node at: ",source_pos," line=",line," ofs=",ofs,"=============");
				let node = find_ast_node::find(dc.crate,source_pos);
				let node_info =  find_ast_node::get_node_info_str(dc,node);
				dump!(node_info);
				// TODO - get infered type from ctxt.node_types??
				// node_id = get_node_id()
				// node_type=ctxt.node_types./*node_type_table*/.get...
				println("node ast loc:"+(do node.map |x| { option_to_str(&x.get_id()) }).to_str());
				match node.last().ty_node_id() {
					None=>{logi!("typeinfo:-unknown node id")},
					Some(id)=> {
						dump_node_source(source_text, node_spans, id);
						match(find_ast_node::safe_node_id_to_type(dc.tycx, id)) {
							None=> logi!("typeinfo:unknown node_type for ",id),
							Some(t)=>{
								println(fmt!("typeinfo: %?",
									{let ntt= rustc::middle::ty::get(t); ntt}));
								dump!(id,dc.tycx.def_map.find(&id));
							},
						};
						let (def_id,opt_span)= def_span_from_node_id(dc,node_spans,id); 
						match(opt_span) {
							None=>{logi!("no def found");}
							Some(sp)=>{
								let BytePos(sp_lo)=sp.lo;
								let def_line_col=text_offset_to_line_pos(source_text,sp_lo);
								logi!("src node=",id," def node=",def_id,
									" span=",sp.my_to_str());
								dump_span(source_text, sp);
							},
						}
					},
				}
			},
		}		
		source_pos+=11;
	}
}

pub fn dump_node_source(text:&[u8], ns:&NodeSpans, id:ast::node_id) {
	match(ns.find(&id)) {None=>logi!("()"),
		Some(span)=>{
			dump_span(text, span);
		}
	}
}

pub fn dump_span(text:&[u8], sp:&codemap::span) {
	let BytePos(x)=sp.lo;
	let line_col=text_offset_to_line_pos(text, x);
	logi!(" line,ofs=",option_to_str(&line_col)," text=\"",
		std::str::from_bytes(text_span(text,sp)),"\"");
}

macro_rules! if_valid{
	($a:expr,$e:expr)=>
	(match a {
		Some(aa)=>e,
		None=>_
	}
	)
}
fn def_span_from_node_id<'a,'b>(dc:&'a DocContext, node_spans:&'b NodeSpans, id:ast::node_id)->(int,Option<&'b codemap::span>) {
	let crate_num=0;
	match dc.tycx.def_map.find(&id) { // finds a def..
		Some(a)=>{
			match get_def_id(crate_num,*a){
				Some(b)=>(b.node,node_spans.find(&b.node)),
				None=>(id as int,None)
			}
		},
		None=>(id as int,None)
	}
	
}

// see: tycx.node_types:node_type_table:HashMap<id,t>
// 't'=opaque ptr, ty::get(:t)->t_box_ to resolve it

fn dump_ctxt_def_map(dc:&DocContext) {
//	let a:()=ctxt.tycx.node_types
	logi!("===Test ctxt def-map table..===");
	for dc.tycx.def_map.iter().advance |(key,value)|{
		dump!(key,value);
	}
}

fn text_line_pos_to_offset(text:&[u8], (line,ofs_in_line):(uint,uint))->Option<uint> {
	// line as reported by grep & text editors,counted from '1' not '0'
	let mut pos = 0;
	let tlen=text.len();	
	let	mut tline=0;
	let mut line_start_pos=0;
	while pos<tlen{
		match text[pos] as char{
			'\n' => {tline+=1; line_start_pos=pos;},
//			"\a" => {tpos=0;line_pos=pos;},
			_ => {}
		}
		// todo - clamp line end
		if tline==(line-1){ 
			return Some(line_start_pos + ofs_in_line);
		}
		pos+=1;
	}
	return None;
}

fn text_offset_to_line_pos(text:&[u8], src_ofs:uint)->Option<(uint,uint)> {
	// line as reported by grep & text editors,counted from '1' not '0'
	let mut pos = 0;
	let tlen=text.len();	
	let	mut tline=0;
	let mut line_start_pos=0;
	while pos<tlen{
		match text[pos] as char{
			'\n' => {
				if src_ofs<=pos && src_ofs>line_start_pos {
					return Some((tline+1,src_ofs-line_start_pos));
				}
				tline+=1; line_start_pos=pos;
			},
//			"\a" => {tpos=0;line_pos=pos;},
			_ => {}
		}
		// todo - clamp line end
		pos+=1;
	}
	return None;
}

fn text_span<'a,'b>(text:&'a [u8],s:&'b codemap::span)->&'a[u8] {
	let codemap::BytePos(lo)=s.lo;
	let codemap::BytePos(hi)=s.hi;
	text.slice(lo,hi)
}




