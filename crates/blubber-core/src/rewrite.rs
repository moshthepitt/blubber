//! Count-preserving, scratch-free collection replacement slots.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;
#[cfg(verus_only)]
use crate::fixed_edit::overwrite_frame;

verus! {

/// Detached admission for a direct collection at the root's live tail.
/// The source must remain exclusively owned and unchanged until `prepare`.
#[doc(hidden)]
pub struct RewriteTail {
    start:u32,
    used:u32,
    selected:Layout,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original:Ghost<Seq<u8>>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    root:Ghost<Layout>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    path:Ghost<InlinePath>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    selection:Ghost<Selection>,
}

pub closed spec fn tail_checked(t:RewriteTail,before:Seq<u8>)->bool {
    let s=t.selection@;
    &&& t.original@==before && !t.path@.overflow && t.path@.len==1
    &&& 12<=t.used<=before.len()<=u32::MAX && words::le_word(before,8)==t.used
    &&& selection_exact(s,before.subrange(12,t.used as int),t.root@,t.path@.steps[0])
    &&& s.run.count==0 && t.start==12+s.span.start && 12+s.span.end==t.used
    &&& t.selected==s.layout && matches!(s.layout.head(),LayoutHead::List{..}|LayoutHead::Map{..})
}
pub closed spec fn tail_for(t:RewriteTail,before:Seq<u8>,root:Layout,path:InlinePath)->bool {
    tail_checked(t,before) && t.root@==root && t.path@==path
}
pub closed spec fn tail_selection(t:RewriteTail)->(Span,Layout) {
    (Span{start:t.start,end:t.used},t.selected)
}
pub closed spec fn tail_write(t:RewriteTail,w:StringWrite,input:u32)->bool {
    &&& tail_checked(t,w.original@) && w.bytes@==w.original@
    &&& !w.issued && w.start==t.start && w.used==t.used
    &&& w.end==t.start+input && w.new_used==w.end
    &&& w.end<=w.bytes.len()
}

/// Resolves a direct tail collection without retaining an ancestor array.
/// Other paths and fields return `None` for the ordinary subtree planner.
#[doc(hidden)]
#[inline(always)]
pub fn resolve_rewrite_tail(bytes:&[u8],discriminator:[u8;8],minimum:u32,layout:&Layout,path:&InlinePath)
    -> (result:Result<Option<RewriteTail>>)
    ensures result.is_ok() && result.unwrap().is_some() ==> tail_for(result.unwrap().unwrap(),bytes@,*layout,*path),
{
    if path.overflow || path.len!=1 {return Ok(None);}
    let id=match path.steps[0] {Step::Field(id)=>id,_=>return Ok(None)};
    let direct=if let Layout::Flat{checked}=layout {
        checked.witness();
        if checked.header(0)==9 && checked.header(4)==1 {
            let used=tail_edit::account_used(bytes,discriminator,minimum)?;
            let local=&bytes[12..used as usize];
            let p=crate::struct_layout::partition_struct_header(local,layout)?;
            let (declaration,_,child)=checked.field(0,true);
            if declaration==id {
                let ordinal=checked.header(3);
                let s=Selection{payload_len:p.payload.end-p.payload.start,span:p.payload,
                    layout:Layout::Flat{checked:child},ordinal,run:PatchRun{offset:0,stride:4,count:0}};
                proof{reveal(crate::layout_head::head_spec);assert(selection_exact(s,local@,*layout,Step::Field(id)));}
                Some((s,used))
            }else{None}
        }else{None}
    }else{None};
    let (s,used)=match direct {
        Some(pair)=>pair,
        None=>match tail_edit::direct_field(bytes,discriminator,minimum,layout,path)? {
            Some(pair)=>pair,None=>return Ok(None),
        },
    };
    if !matches!(s.layout.tag(),7|8) || s.run.count!=0 || 12+s.span.end as usize!=used as usize {
        return Ok(None);
    }
    proof {reveal(crate::layout_head::head_spec);}
    Ok(Some(RewriteTail{start:(12+s.span.start as usize) as u32,used,selected:s.layout,
        original:Ghost(bytes@),root:Ghost(*layout),path:Ghost(*path),selection:Ghost(s)}))
}
impl RewriteTail {
    /// Returns the exact source interval and descriptor retained at admission.
    #[inline(always)]
    pub fn selection(&self)->(result:(Span,Layout))
        ensures result==tail_selection(*self),
    {(Span{start:self.start,end:self.used},self.selected)}

    /// Prepares delayed publication within initialized capacity.
    /// `None` requests the general reserve and checked reacquisition path.
    #[inline(always)]
    pub fn prepare<'a>(self,bytes:&'a mut[u8],input:u32)->(result:Result<Option<StringWrite<'a>>>)
        requires tail_checked(self,old(bytes)@),
        ensures result.is_err() || result.unwrap().is_none() ==> final(bytes)@==old(bytes)@,
            result.is_ok() && result.unwrap().is_some() ==> tail_write(self,result.unwrap().unwrap(),input),
    {
        let end=checked_delta(self.start,input as i64)?;
        if end as usize>bytes.len(){return Ok(None);}
        Ok(Some(string_edit::direct_write(bytes,self.start,self.used,end)))
    }
}
/// Detached coordinates for rewriting every value while retaining count and keys.
///
/// The caller must keep exclusive ownership between checking the source and
/// beginning the stream. Only backing-allocation relocation and one enclosing
/// subtree suffix move may intervene; count and key bytes must survive both.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RewritePlan {
    pub(super) count: u32,
    pub(super) payload: u32,
    pub(super) total: u32,
    pub(super) len: u32,
    pub(super) key: u32,
    pub(super) stride: u32,
    pub(super) width: Option<u32>,
    pub(super) map: bool,
}

pub(super) open spec fn coordinates(c: RewritePlan) -> bool {
    &&& 4 <= c.payload <= c.len <= u32::MAX
    &&& c.count == 0 ==> c.total == 0
    &&& c.width.is_some() ==> c.total == c.count * c.width.unwrap()
    &&& !c.map ==> c.key == 0
    &&& c.map ==> c.key > 0
    &&& if c.width.is_some() {
        c.payload == 4 && (c.count>0 ==> c.stride == c.key + c.width.unwrap())
            && c.len == 4 + c.count * c.key + c.total
    } else {
        (c.count>0 ==> c.stride == c.key + 4)
            && c.payload == 4 + (if c.map { c.count as int } else { if c.count == 0 {0int} else {c.count - 1} }) * c.stride
            && c.len == c.payload + c.total
    }
}

pub(super) open spec fn corresponds(c: RewritePlan, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::List { .. } => !c.map && c.width == crate::list_layout::element_width(layout),
        LayoutHead::Map { .. } => c.map && c.key == crate::map_layout::key_width(layout)
            && c.width == crate::map_layout::value_width(layout),
        _ => false,
    }
}

pub(super) open spec fn slot_start(c: RewritePlan, done: int, end: int) -> int {
    c.payload + end + if c.width.is_some() { (done + 1) * c.key } else { 0int }
}

pub(super) open spec fn key_at(c:RewritePlan,i:int) -> int {4+i*c.stride}

pub(super) open spec fn key_byte(c: RewritePlan, p: int) -> bool {
    c.map && exists|i: int| 0 <= i < c.count && #[trigger] key_at(c,i) <= p < key_at(c,i) + c.key
}

pub(super) open spec fn header_frame(before: Seq<u8>, after: Seq<u8>, c: RewritePlan) -> bool {
    &&& 4 <= before.len() && after.len() == c.len
    &&& (if c.width.is_some() {c.len as int} else {c.payload as int}) <= before.len()
    &&& forall|p: int| 0 <= p < 4 || key_byte(c,p) ==> #[trigger] after[p] == before[p]
}

pub(super) open spec fn prefix_ends(c: RewritePlan, ends: Seq<int>) -> bool {
    &&& 1 <= ends.len() <= c.count + 1 && ends[0] == 0
    &&& forall|i: int| 0 <= i < ends.len() ==> 0 <= #[trigger] ends[i] <= c.total
    &&& forall|i: int| 0 <= i < ends.len()-1 ==> #[trigger] ends[i] <= ends[i+1]
    &&& c.width.is_some() ==> forall|i: int| 0 <= i < ends.len() ==> #[trigger] ends[i] == i * c.width.unwrap()
}

pub(super) open spec fn stored(c: RewritePlan, i: int) -> bool {
    c.width.is_none() && (c.map || i + 1 < c.count)
}

pub(super) open spec fn directory(bytes: Seq<u8>, c: RewritePlan, ends: Seq<int>) -> bool {
    forall|i: int| 0 <= i < ends.len()-1 && stored(c,i)
        ==> #[trigger] words::le_word(bytes,4+i*c.stride+c.key) == ends[i+1]
}

pub(super) open spec fn rewrite_result(before: Seq<u8>, after: Seq<u8>, c: RewritePlan, ends: Seq<int>) -> bool {
    coordinates(c) && words::le_word(before,0)==c.count && header_frame(before,after,c) && prefix_ends(c,ends)
        && ends.len() == c.count+1 && ends[c.count as int] == c.total && directory(after,c,ends)
}

pub closed spec fn plan_checked(c: RewritePlan) -> bool { coordinates(c) }
pub closed spec fn plan_len(c: RewritePlan) -> u32 { c.len }
pub closed spec fn plan_empty(c:RewritePlan) -> bool {c.count==0}
pub closed spec fn typed_result(result:Result<RewritePlan>,c:RewritePlan,width:Option<u32>,key:u32,map:bool)->bool {
    &&& result.is_ok() ==> result.unwrap()==c && plan_checked(result.unwrap())
    &&& result.is_ok() == (c.width==width && c.key==key && c.map==map)
}
pub closed spec fn check_result(result: Result<RewritePlan>, bytes: Seq<u8>, layout: Layout, count: u32,total:u32) -> bool {
    result.is_ok() ==> {
        let c = result.unwrap();
        &&& coordinates(c) && corresponds(c,layout)
        &&& c.count == count && c.total == total
        &&& count == words::le_word(bytes,0)
        &&& validation::local_valid(bytes,layout,true)
    }
}

/// Geometry admission deliberately carries no key-order or domain certificate.
pub closed spec fn fixed_map_result(result:Result<RewritePlan>,bytes:Seq<u8>,layout:Layout,count:u32,total:u32)->bool {
    &&& result.is_ok() == (crate::layout::descriptor_valid(layout,MAX_DEPTH as nat)
        && crate::map_layout::value_width(layout).is_some()
        && crate::map_layout::map_shape(bytes,layout)
        && count==words::le_word(bytes,0)
        && total==count*crate::map_layout::value_width(layout).unwrap())
    &&& result.is_ok() ==> {
        let c=result.unwrap();
        &&& coordinates(c) && corresponds(c,layout) && c.map
        &&& c.count==count && c.total==total && c.len==bytes.len()
        &&& count==words::le_word(bytes,0)
        &&& crate::map_layout::map_shape(bytes,layout)
    }
}

/// Valid history supplies semantic evidence absent from geometry admission.
/// The unchanged rewrite publication theorem then composes exact slot encoders
/// with ancestor frames to preserve the enclosing complete root.
#[verifier::spinoff_prover]
proof fn rewrite_fixed_map_history(result:Result<RewritePlan>,bytes:Seq<u8>,layout:Layout,count:u32,total:u32)
    requires fixed_map_result(result,bytes,layout,count,total),
        validation::local_valid(bytes,layout,true),
    ensures check_result(result,bytes,layout,count,total),
{
}

/// A nonempty production admission inhabits the conditional history contract.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn rewrite_fixed_map_witness()
    ensures true,
{
    let bytes=[2u8,0,0,0,1,0,3,1];
    let layout=witnesses::FIXED_MAP;
    let result=RewritePlan::check_fixed_map(&bytes,&layout,2,2);
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid,6);
        assert(bytes@==seq![2u8,0,0,0,1,0,3,1]);
        assert(crate::search::same_prefix(seq![1u8],seq![3u8],0));
        assert(crate::search::lex_less(seq![1u8],seq![3u8]));
        assert(result.is_ok());
        assert(crate::search::encoded_key(bytes@,layout,0)==seq![1u8]);
        assert(crate::search::encoded_key(bytes@,layout,1)==seq![3u8]);
        assert(crate::search::keys_canonical(bytes@,layout,2));
        assert(crate::search::keys_ordered(bytes@,layout,2));
        assert(validation::local_valid(bytes@,layout,true));
        rewrite_fixed_map_history(result,bytes@,layout,2,2);
    }
}

impl RewritePlan {
    /// Admits an exact fixed-map extent without inspecting stored keys or values.
    ///
    /// Count, stride, output size and every slot bound are checked. This grants
    /// no key-order or domain evidence. Complete-root preservation additionally
    /// requires an initially valid root and valid encoded replacement values.
    #[inline(always)]
    pub fn check_fixed_map(bytes: &[u8], layout: &Layout, count: u32, total: u32)
        -> (result: Result<Self>)
        ensures fixed_map_result(result,bytes@,*layout,count,total),
    {
        let p = crate::map_layout::partition_map_header(bytes,layout)?;
        let width = match p.value_width {
            Some(width) => width,
            None => return Err(ErrorKind::InvalidSchema.into()),
        };
        if count != p.count || total != checked_u32(checked_mul(count as usize,width as usize)?)? {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        proof {
            if count>0 {
                assert(p.key_width as int+width<=bytes.len()) by(nonlinear_arith)
                    requires count>0,bytes.len()==4+count*(p.key_width as int+width);
            }
        }
        let stride = if count == 0 {0} else {
            checked_u32(checked_add(p.key_width as usize,width as usize)?)?
        };
        let len = checked_u32(bytes.len())?;
        let c = Self {count,payload:4,total,len,key:p.key_width,stride,width:Some(width),map:true};
        proof {
            assert(crate::map_layout::map_shape(bytes@,*layout));
            assert(len == 4 + count * p.key_width + total) by(nonlinear_arith)
                requires len == 4 + count * (p.key_width + width), total == count * width;
        }
        Ok(c)
    }

    /// Checks local structure and map key domains/order, count and total arithmetic.
    /// `total` is the sum of encoded value lengths, excluding keys and directory.
    /// Child domains remain the encoder's obligation. No bytes are changed.
    pub fn check(bytes: &[u8], layout: &Layout, count: u32, total: u32) -> (result: Result<Self>)
        ensures check_result(result,bytes@,*layout,count,total),
    {
        let parts = enter(bytes,layout,true)?;
        let (actual,payload,key,width,map) = match parts {
            Parts::List(p) => (p.count,p.payload.start,0u32,p.element_width,false),
            Parts::Map(p) => (p.count,p.payload.start,p.key_width,p.value_width,true),
            _ => return Err(Error::new(ErrorKind::InvalidSchema,Error::NO_OFFSET)),
        };
        if count != actual || (count == 0 && total != 0) {
            return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));
        }
        if let Some(w) = width {
            let expected = checked_u32(checked_mul(count as usize,w as usize)?)?;
            if total != expected { return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)); }
        }
        let stride = if count==0 {0} else {checked_u32(checked_add(key as usize, width.unwrap_or(4) as usize)?)?};
        let len = if width.is_some() {
            checked_u32(checked_add(4,checked_add(checked_mul(count as usize,key as usize)?,total as usize)?)?)?
        } else { checked_u32(checked_add(payload as usize,total as usize)?)? };
        let c=Self{count,payload,total,len,key,stride,width,map};
        proof {
            assert(corresponds(c,*layout));
            if width.is_none() {
                if map {
                    assert(payload as int==crate::map_layout::payload_start(bytes@,*layout));
                    if count>0 {assert(crate::map_layout::entry_stride(*layout)==stride);}
                } else {
                    assert(payload as int==crate::list_layout::payload_start(bytes@,*layout));
                    if count>0 {assert(stride==4);}
                    assert(crate::struct_layout::directory_size(count as int)==if count==0{0int}else{4*(count as int-1)});
                }
            }
            if width.is_none() {
                if count==0 {assert(payload==4);}
                else if map {
                    assert(payload as int==4+count as int*crate::map_layout::entry_stride(*layout));
                    assert(payload as int==4+count as int*stride) by(nonlinear_arith)
                        requires payload as int==4+count as int*crate::map_layout::entry_stride(*layout),
                            crate::map_layout::entry_stride(*layout)==stride;
                }else{
                    assert(payload as int==4+4*(count as int-1));
                    assert(payload as int==4+(count as int-1)*stride) by(nonlinear_arith)
                        requires payload as int==4+4*(count as int-1),stride==4;
                }
            }
            assert(coordinates(c));
        }
        Ok(c)
    }

    /// Builds geometry from an already locally valid exact source.
    ///
    /// The caller must have checked this slice's structure and map key domains
    /// and order against this layout, retaining exclusive ownership since then.
    /// Only header geometry, count and output arithmetic are checked here.
    /// Use `check` when that source-byte evidence is unavailable.
    #[inline(always)]
    pub fn check_admitted(bytes: &[u8], layout: &Layout, count: u32, total: u32) -> (result: Result<Self>)
        requires validation::local_valid(bytes@,*layout,true),
        ensures check_result(result,bytes@,*layout,count,total),
    {
        let parts = crate::edit_layout::enter_local(bytes,layout,false)?;
        proof {assert(validation::parts_valid(parts,bytes@,*layout,true));}
        let (actual,payload,key,width,map) = match parts {
            Parts::List(p) => (p.count,p.payload.start,0u32,p.element_width,false),
            Parts::Map(p) => (p.count,p.payload.start,p.key_width,p.value_width,true),
            _ => return Err(Error::new(ErrorKind::InvalidSchema,Error::NO_OFFSET)),
        };
        if count != actual || (count == 0 && total != 0) {
            return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));
        }
        if let Some(w) = width {
            let expected = checked_u32(checked_mul(count as usize,w as usize)?)?;
            if total != expected { return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)); }
        }
        let stride = if count==0 {0} else {checked_u32(checked_add(key as usize, width.unwrap_or(4) as usize)?)?};
        let len = if width.is_some() {
            checked_u32(checked_add(4,checked_add(checked_mul(count as usize,key as usize)?,total as usize)?)?)?
        } else { checked_u32(checked_add(payload as usize,total as usize)?)? };
        let c=Self{count,payload,total,len,key,stride,width,map};
        proof {
            assert(corresponds(c,*layout));
            if width.is_none() {
                if map {
                    assert(payload as int==crate::map_layout::payload_start(bytes@,*layout));
                    if count>0 {assert(crate::map_layout::entry_stride(*layout)==stride);}
                } else {
                    assert(payload as int==crate::list_layout::payload_start(bytes@,*layout));
                    if count>0 {assert(stride==4);}
                    assert(crate::struct_layout::directory_size(count as int)==if count==0{0int}else{4*(count as int-1)});
                }
            }
            if width.is_none() {
                if count==0 {assert(payload==4);}
                else if map {
                    assert(payload as int==4+count as int*crate::map_layout::entry_stride(*layout));
                    assert(payload as int==4+count as int*stride) by(nonlinear_arith)
                        requires payload as int==4+count as int*crate::map_layout::entry_stride(*layout),
                            crate::map_layout::entry_stride(*layout)==stride;
                }else{
                    assert(payload as int==4+4*(count as int-1));
                    assert(payload as int==4+(count as int-1)*stride) by(nonlinear_arith)
                        requires payload as int==4+4*(count as int-1),stride==4;
                }
            }
            assert(coordinates(c));
        }
        Ok(c)
    }

    /// Checks typed codec geometry while retaining the admitted plan unchanged.
    /// The explicit constants let SBF specialize exact-slot encoding loops.
    #[doc(hidden)]
    #[inline(always)]
    pub fn typed(self,width:Option<u32>,key:u32,map:bool)->(result:Result<Self>)
        requires plan_checked(self),
        ensures typed_result(result,self,width,key,map),
        no_unwind
    {
        if self.width!=width || self.key!=key || self.map!=map {
            return Err(Error::new(ErrorKind::InvalidSchema,Error::NO_OFFSET));
        }
        Ok(Self{width,key,map,..self})
    }

    /// Exact complete collection length to pass to the enclosing subtree planner.
    #[inline(always)]
    pub fn encoded_len(&self) -> (len: u32)
        ensures len == plan_len(*self),
    { self.len }

    /// Whether checked count preservation makes this a write-free empty operation.
    #[inline(always)]
    pub fn is_empty(&self) -> (empty:bool)
        ensures empty==plan_empty(*self),
    {self.count==0}

    /// Starts a stream in the final subtree slot, with the old count/keys retained.
    /// A wrong destination length is rejected before writing.
    #[inline(always)]
    pub fn begin<'a>(self, bytes: &'a mut [u8]) -> (result: Result<RewriteStream<'a>>)
        requires plan_checked(self),
        ensures result.is_ok() ==> stream_for(result.unwrap(),old(bytes)@,self),
            result.is_err() ==> final(bytes)@ == old(bytes)@,
        no_unwind
    {
        if bytes.len() != self.len as usize { return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)); }
        if indexed_walk::indexed_read_word(bytes,0) != self.count { return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)); }
        let ghost before=bytes@;
        let ghost ends=seq![0int];
        proof {
            assert forall|i:int|0<=i<ends.len() implies 0<=#[trigger] ends[i]<=self.total by {assert(i==0);}
            assert forall|i:int|0<=i<ends.len() && self.width.is_some() implies #[trigger] ends[i]==i*self.width.unwrap() by {assert(i==0);}
            assert(prefix_ends(self,ends));
        }
        let s=RewriteStream { original: Ghost(bytes@), bytes, plan:self, done:0,end:0,pending:None,issued:false,ends:Ghost(ends) };
        proof {assert(header_frame(before,s.bytes@,self));assert(s.valid());}
        Ok(s)
    }
}

/// Exclusive two-pass rewrite output; no old payload or length scratch is retained.
///
/// For each child call `prepare_slot`, encode its `destination`, then `advance` with the
/// encoder's reported byte count. Only `finish` permits enclosing publication.
/// Any error or unfinished stream requires the enclosing store to remain poisoned.
#[doc(hidden)]
#[must_use = "finish every exact slot before publishing the enclosing subtree"]
pub struct RewriteStream<'a> {
    bytes: &'a mut [u8],
    plan: RewritePlan,
    done: usize,
    end: usize,
    pending: Option<u32>,
    issued: bool,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
    ends: Ghost<Seq<int>>,
}

pub closed spec fn stream_checked(s: RewriteStream) -> bool { s.valid() }
pub closed spec fn stream_for(s: RewriteStream, before: Seq<u8>, c: RewritePlan) -> bool {
    s.valid() && s.original@ == before && s.bytes@ == before && s.plan == c
        && s.done == 0 && s.end == 0 && s.pending.is_none()
}
pub closed spec fn stream_storage<'a>(s: RewriteStream<'a>) -> &'a mut [u8] { s.bytes }
pub closed spec fn stream_finished(s: RewriteStream, result: Result<()>) -> bool {
    &&& result.is_ok() == (s.pending.is_none() && s.done==s.plan.count && s.end==s.plan.total)
    &&& result.is_ok() ==> rewrite_result(s.original@,s.bytes@,s.plan,s.ends@)
}

pub closed spec fn slot_prepared(s:RewriteStream,after:RewriteStream,result:Result<Span>,len:u32) -> bool {
    &&& after.bytes@==s.bytes@ && after.plan==s.plan && after.original==s.original && after.ends==s.ends
    &&& after.done==s.done && after.end==s.end
    &&& result.is_ok() ==> after.pending==Some(len) && !after.issued && stream_available(after)
        && result.unwrap()==(Span {start:slot_start(s.plan,s.done as int,s.end as int) as u32,
            end:(slot_start(s.plan,s.done as int,s.end as int)+len) as u32})
    &&& result.is_err() ==> after.pending==s.pending && after.issued==s.issued
}
pub closed spec fn stream_available(s:RewriteStream) -> bool {s.valid() && s.pending.is_some() && !s.issued}
pub closed spec fn stream_loan(s:RewriteStream,after:RewriteStream,initial:Seq<u8>,encoded:Seq<u8>) -> bool {
    let start=slot_start(s.plan,s.done as int,s.end as int);
    let len=s.pending.unwrap();
    &&& initial.len()==len && initial==s.bytes@.subrange(start,start+len)
    &&& after.bytes@.subrange(start,start+len)==encoded
    &&& after.plan==s.plan && after.ends==s.ends && after.original==s.original
    &&& after.done==s.done && after.end==s.end && after.pending==s.pending && after.issued
    &&& overwrite_frame(s.bytes@,after.bytes@,Span {start:start as u32,end:(start+len) as u32})
}
pub closed spec fn advance_result(s:RewriteStream,after:RewriteStream,result:Result<()>,written:usize) -> bool {
    &&& result.is_ok() == (s.pending.is_some() && s.issued && written==s.pending.unwrap())
    &&& result.is_ok() ==> if stored(s.plan,s.done as int) {
        let at=4+s.done*s.plan.stride+s.plan.key;
        overwrite_frame(s.bytes@,after.bytes@,Span {start:at as u32,end:(at+4) as u32})
    } else {after.bytes@==s.bytes@}
    &&& result.is_err() ==> after.bytes@ == s.bytes@
    &&& result.is_ok() ==> after.done == s.done+1 && after.pending.is_none() && !after.issued
        && after.ends@ == s.ends@.push(after.end as int)
}

pub(super) open spec fn stream_shape(c:RewritePlan,original:Seq<u8>,bytes:Seq<u8>,ends:Seq<int>,done:usize,end:usize,pending:Option<u32>,issued:bool) -> bool {
    &&& coordinates(c)
    &&& header_frame(original,bytes,c)
    &&& words::le_word(original,0)==c.count
    &&& prefix_ends(c,ends)
    &&& ends.len()==done+1 && end==ends[done as int]
    &&& directory(bytes,c,ends)
    &&& issued ==> pending.is_some()
    &&& pending.is_some() ==> done<c.count && pending.unwrap()<=c.total-end
        && (c.width.is_some() ==> pending==c.width)
}

pub closed spec fn fixed_fill_result(before: RewriteStream, after: RewriteStream, result: Result<bool>) -> bool {
    &&& stream_checked(after) && after.plan == before.plan && after.original == before.original
    &&& result == Ok(true) ==> after.done == after.plan.count
        && after.end == after.plan.total && after.pending.is_none()
}

/// Uniform fills retain checked geometry and require a complete successful stream.
pub closed spec fn uniform_fill_result(before: RewriteStream, after: RewriteStream, result: Result<()>) -> bool {
    &&& stream_checked(after) && after.plan == before.plan && after.original == before.original
    &&& result.is_ok() ==> after.done == after.plan.count
        && after.end == after.plan.total && after.pending.is_none()
}

impl<'a> RewriteStream<'a> {
    closed spec fn valid(self) -> bool {
        stream_shape(self.plan,self.original@,self.bytes@,self.ends@,self.done,self.end,self.pending,self.issued)
    }

    /// Fills a fresh indexed stream through paired payload/directory loans.
    /// Each successful encoding publishes its end before the next callback.
    /// Errors and unwinds retain the exact completed prefix under owner poison.
    #[doc(hidden)]
    #[inline(always)]
    #[verifier::spinoff_prover]
    #[verifier::rlimit(180)]
    pub fn fill_uniform<E: crate::fill::ValueEncoder>(&mut self, len: u32, key: u32, encoder: &E)
        -> (result: Result<()>)
        requires stream_checked(*old(self)),
        ensures uniform_fill_result(*old(self), *final(self), result),
    {
        let c=self.plan;
        if c.width.is_some() || key!=c.key || self.done!=0 || self.pending.is_some() {
            return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));
        }
        let total=checked_u32(checked_mul(c.count as usize,len as usize)?)?;
        if total!=c.total {return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));}
        if c.count==0 {return Ok(());}
        proof {
            assert(len<=c.total) by(nonlinear_arith) requires c.count>0,c.total==c.count*len;
            if c.map {
                assert(key+4<=u32::MAX) by(nonlinear_arith)
                    requires c.payload==4+c.count*(key+4),c.count>0,c.payload<=u32::MAX;
            }
            assert(c.payload-4==crate::fill::uniform_stores(c.count as int,key as int)*(key+4)) by(nonlinear_arith)
                requires c.payload==4+(if c.map {c.count as int}else{c.count-1})*c.stride,
                    c.stride==key+4,c.map==(key>0);
        }
        self.pending=Some(len);self.issued=true;
        let ghost before=self.bytes@;
        let ghost mut after_directory=Seq::<u8>::empty();
        let (done,outcome)={
            let (header,payload)=self.bytes.split_at_mut(c.payload as usize);
            let (_,directory)=header.split_at_mut(4);
            let result=crate::fill::uniform_stream(directory,payload,len,c.count,key,encoder);
            proof {after_directory=directory@;}
            result
        };
        let ghost ends=Seq::new(done as nat+1,|i:int|i*len);
        proof {
            assert(self.bytes@.subrange(4,c.payload as int)==after_directory);
            assert forall|p:int|0<=p<4 || key_byte(c,p) implies #[trigger] self.bytes@[p]==before[p] by {
                if key_byte(c,p) {
                    rewrite_key_bounds(c,p);
                    let i=choose|i:int|0<=i<c.count && #[trigger] key_at(c,i)<=p<key_at(c,i)+c.key;
                    vstd::arithmetic::div_mod::lemma_mod_multiples_basic(i,c.stride as int);
                    vstd::arithmetic::div_mod::lemma_mod_adds(i*c.stride,p-4-i*c.stride,c.stride as int);
                    vstd::arithmetic::div_mod::lemma_small_mod((p-4-i*c.stride) as nat,c.stride as nat);
                }
            }
            assert(header_frame(self.original@,self.bytes@,c));
            assert forall|i:int|0<=i<done && stored(c,i) implies #[trigger] words::le_word(self.bytes@,4+i*c.stride+c.key)==ends[i+1] by {
                rewrite_end_bounds(c,i);
                crate::fill::uniform_word_window(self.bytes@,after_directory,4,i*c.stride+c.key);
            }
            assert(directory(self.bytes@,c,ends));
            assert forall|i:int|0<=i<ends.len() implies 0<=#[trigger] ends[i]<=c.total by {
                assert(0<=i*len<=c.count*len) by(nonlinear_arith) requires 0<=i<=c.count;
            }
            assert forall|i:int|0<=i<ends.len()-1 implies #[trigger] ends[i]<=ends[i+1] by {
                assert(i*len<=(i+1)*len) by(nonlinear_arith);
            }
            assert(prefix_ends(c,ends));
            assert(done*len<=c.total) by(nonlinear_arith) requires done<=c.count,c.total==c.count*len;
        }
        self.done=done;
        self.end=indexed_walk::indexed_coordinate(0,done,len as usize);
        self.ends=Ghost(ends);
        if outcome.is_ok(){self.pending=None;self.issued=false;}
        proof {
            if outcome.is_err() {
                assert(len<=c.total-self.end) by(nonlinear_arith)
                    requires done<c.count,self.end==done*len,c.total==c.count*len;
            }
        }
        outcome
    }

    /// Fills a fresh fixed stream through exact value loans in one pass.
    /// Returns `false` for indexed or zero-width values, retaining the stream.
    /// Encoder failures leave an unfinished slot and require sticky poison.
    #[doc(hidden)]
    #[inline(always)]
    #[verifier::spinoff_prover]
    #[verifier::rlimit(180)]
    pub fn fill_fixed<E: crate::fill::ValueEncoder>(&mut self, width: Option<u32>, key: u32, encoder: &E)
        -> (result: Result<bool>)
        requires stream_checked(*old(self)),
        ensures fixed_fill_result(*old(self), *final(self), result),
    {
        if width.is_none() || width == Some(0) { return Ok(false); }
        let width = width.unwrap();
        let geometry = encoder.geometry();
        if geometry != (width,key) { return Err(Error::new(ErrorKind::InvalidSchema,Error::NO_OFFSET)); }
        let c = self.plan;
        if c.width != Some(width) || c.key != key || self.done != 0 || self.pending.is_some() {
            return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET));
        }
        if c.count == 0 { return Ok(true); }
        proof {
            assert(width + key == c.stride);
            assert(c.len == 4 + c.count * c.stride) by(nonlinear_arith)
                requires c.len == 4 + c.count * c.key + c.count * width, c.stride == c.key + width;
            vstd::arithmetic::div_mod::lemma_mod_multiples_basic(c.count as int, c.stride as int);
        }
        proof {
            assert(width <= c.total) by(nonlinear_arith)
                requires c.count > 0, c.total == c.count * width, width > 0;
        }
        self.pending = Some(width);
        self.issued = true;
        let ghost before = self.bytes@;
        // control: rewrite-key-frame
        let result = crate::fill::values(&mut self.bytes[4..], encoder);
        proof {
            assert forall|p: int| 0 <= p < 4 || key_byte(c,p)
                implies #[trigger] self.bytes@[p] == before[p] by {
                if key_byte(c,p) {
                    rewrite_key_bounds(c,p);
                    let i = choose|i: int| 0 <= i < c.count && #[trigger] key_at(c,i) <= p < key_at(c,i) + c.key;
                    vstd::arithmetic::div_mod::lemma_mod_multiples_basic(i, c.stride as int);
                    vstd::arithmetic::div_mod::lemma_mod_adds(
                        i * c.stride, p - 4 - i * c.stride, c.stride as int);
                    vstd::arithmetic::div_mod::lemma_small_mod((p - 4 - i * c.stride) as nat, c.stride as nat);
                }
            }
        }
        proof {
            assert(header_frame(self.original@,self.bytes@,c));
            assert(directory(self.bytes@,c,self.ends@));
            assert(self.end == 0);
            assert(self.pending.unwrap() <= c.total-self.end);
            assert(self.valid());
        }
        result?;
        self.done = c.count as usize;
        self.end = c.total as usize;
        self.pending = None;
        self.issued = false;
        self.ends = Ghost(Seq::new(c.count as nat + 1, |i: int| i * width));
        proof {
            assert forall|i: int| 0 <= i <= c.count implies 0 <= #[trigger] self.ends@[i] <= c.total by {
                assert(0 <= i * width <= c.total) by(nonlinear_arith)
                    requires 0 <= i <= c.count, c.total == c.count * width, width > 0;
            }
            assert forall|i: int| 0 <= i < self.ends@.len()-1 implies
                #[trigger] self.ends@[i] <= self.ends@[i+1] by {
                assert(i * width <= (i+1)*width) by(nonlinear_arith)
                    requires width > 0;
            }
            assert forall|i: int| 0 <= i < self.ends@.len() implies
                #[trigger] self.ends@[i] == i * c.width.unwrap() by {}
            assert(prefix_ends(c,self.ends@));
        }
        Ok(true)
    }

    /// Checks and stages one exact final slot without writing any byte.
    /// Count, fixed width and remaining measured extent are checked first.
    #[inline(always)]
    pub fn prepare_slot(&mut self,len:u32) -> (result:Result<Span>)
        requires stream_checked(*old(self)),
        ensures stream_checked(*final(self)),slot_prepared(*old(self),*final(self),result,len),
        no_unwind
    {
        let c=self.plan;
        if self.pending.is_some() || self.done==c.count as usize || len as usize>crate::words::sub_bounded(c.total as usize,self.end)
            || (c.width.is_some() && Some(len)!=c.width) {
            return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));
        }
        proof {rewrite_slot_bounds(c,self.done as int,self.end as int,len as int);}
        let key_offset=if c.width.is_some() {indexed_walk::indexed_coordinate(0,crate::words::add_bounded(self.done,1),c.key as usize)}else{0};
        let start=crate::words::add_bounded(c.payload as usize,crate::words::add_bounded(self.end,key_offset));
        self.pending=Some(len);
        Ok(Span {start:start as u32,end:(crate::words::add_bounded(start,len as usize)) as u32})
    }

    /// Lends the staged exact slot once; its end remains unpublished.
    ///
    /// # Panics
    /// Panics unless `prepare_slot` succeeded and this slot has not been issued.
    #[inline(always)]
    pub fn destination(&mut self) -> (slot:&mut [u8])
        requires stream_available(*old(self)),
        ensures stream_checked(*final(self)),stream_loan(*old(self),*final(self),slot@,final(slot)@),
        no_unwind
    {
        hide(words::le_word);
        hide(overwrite_frame);
        let len=match self.pending {Some(len)=>len,None=>panic!("rewrite slot not prepared")};
        if self.issued {panic!("rewrite slot issued twice");}
        let c=self.plan;
        proof {rewrite_slot_bounds(c,self.done as int,self.end as int,len as int);}
        let key_offset=if c.width.is_some() {indexed_walk::indexed_coordinate(0,crate::words::add_bounded(self.done,1),c.key as usize)}else{0};
        let start=crate::words::add_bounded(c.payload as usize,crate::words::add_bounded(self.end,key_offset));
        proof {assert(start as int==slot_start(c,self.done as int,self.end as int));}
        let ghost before=self.bytes@;
        proof {rewrite_loan_frame(self.original@,before,c,self.ends@,self.done as int,self.end as int,len as int);}
        self.issued=true;
        proof {assert(self.bytes@==before);}
        indexed_walk::indexed_slot_region(self.bytes,start,crate::words::add_bounded(start,len as usize))
    }

    /// Checks the encoder count, then stores this child's cumulative end once.
    /// A count mismatch leaves the pending slot unfinished and changes no bytes.
    #[inline(always)]
    #[verifier::spinoff_prover]
    #[verifier::rlimit(120)]
    pub fn advance(&mut self, written: usize) -> (result: Result<()>)
        requires stream_checked(*old(self)),
        ensures stream_checked(*final(self)), advance_result(*old(self),*final(self),result,written),
        no_unwind
    {
        hide(words::le_word);
        let len = match self.pending { Some(len) => len, None => return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)) };
        if !self.issued || written != len as usize { return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)); }
        let c = self.plan;
        let next = crate::words::add_bounded(self.end,len as usize);
        let ghost before = self.bytes@;
        if c.width.is_none() && (c.map || crate::words::add_bounded(self.done,1) < c.count as usize) {
            proof { rewrite_end_bounds(c,self.done as int); }
            let at = indexed_walk::indexed_coordinate(4+c.key as usize,self.done,c.stride as usize);
            proof { rewrite_end_bounds(c,self.done as int); }
            crate::words::write_contained_word(self.bytes,at,next as u32);
        }
        let ghost old_ends=self.ends@;
        proof {
            rewrite_prefix_append(c,old_ends,next as int);
            rewrite_end_frame(self.original@,before,self.bytes@,c,old_ends,self.done as int,next as int);
        }
        self.done=crate::words::add_bounded(self.done,1);
        self.end=next;
        self.pending=None;
        self.issued=false;
        self.ends=Ghost(self.ends@.push(next as int));
        Ok(())
    }

    /// Authorizes publication only after exact count and final-total agreement.
    #[inline(always)]
    pub fn finish(self) -> (result: Result<()>)
        requires stream_checked(self),
        ensures stream_finished(self,result),
        no_unwind
    {
        if self.pending.is_some() || self.done!=self.plan.count as usize || self.end!=self.plan.total as usize {
            Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET))
        } else { Ok(()) }
    }
}

/// Publishing one end frames every key and all previously completed ends.
#[verifier::spinoff_prover]
proof fn rewrite_end_frame(original:Seq<u8>,before:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>,done:int,next:int)
    requires coordinates(c),header_frame(original,before,c),directory(before,c,ends),
        prefix_ends(c,ends),ends.len()==done+1,0<=done<c.count,
        if stored(c,done) {
            let at=4+done*c.stride+c.key;
            words::encode_frame(before,after,at) && words::le_word(after,at)==next
        } else {after==before},
    ensures header_frame(original,after,c),directory(after,c,ends.push(next)),
{
    hide(words::le_word);
    if stored(c,done) {
        let at=4+done*c.stride+c.key;
        rewrite_end_bounds(c,done);
        assert forall|p:int| key_byte(c,p) implies p<at || at+4<=p by {
            let i=choose|i:int|0<=i<c.count && #[trigger] key_at(c,i)<=p<key_at(c,i)+c.key;
            if i<=done {assert(4+i*c.stride+c.key<=at) by(nonlinear_arith)
                requires i<=done,c.stride==c.key+4,c.key>0,at==4+done*c.stride+c.key;}
            else {assert(at+4<=4+i*c.stride) by(nonlinear_arith)
                requires done<i,c.stride==c.key+4,c.key>0,at==4+done*c.stride+c.key;}
        }
        assert forall|p:int|0<=p<4 || key_byte(c,p) implies #[trigger] after[p]==original[p] by {
            if key_byte(c,p) {rewrite_key_bounds(c,p);}
        }
    }
    assert forall|i:int|0<=i<ends.len() && stored(c,i) implies
        #[trigger] words::le_word(after,4+i*c.stride+c.key)==ends.push(next)[i+1] by {
        if i<done {
            if stored(c,done) {
                assert(4+i*c.stride+c.key+4<=4+done*c.stride+c.key) by(nonlinear_arith)
                    requires i<done,c.stride==c.key+4;
            }
            rewrite_end_bounds(c,i);
            rewrite_same_word(before,after,4+i*c.stride+c.key);
        } else {assert(i==done);}
    }
}

/// One successfully encoded slot extends the monotone prefix sum by its length.
#[verifier::spinoff_prover]
proof fn rewrite_prefix_append(c:RewritePlan,ends:Seq<int>,end:int)
    requires prefix_ends(c,ends),ends.len()<=c.count,ends[ends.len()-1]<=end<=c.total,
        c.width.is_some() ==> end==ends[ends.len()-1]+c.width.unwrap(),
    ensures prefix_ends(c,ends.push(end)),
{
    let next=ends.push(end);
    assert forall|i:int|0<=i<next.len() implies 0<=#[trigger] next[i]<=c.total by {}
    assert forall|i:int|0<=i<next.len()-1 implies #[trigger] next[i]<=next[i+1] by {}
    assert forall|i:int|0<=i<next.len() && c.width.is_some() implies #[trigger] next[i]==i*c.width.unwrap() by {
        if i==ends.len() {assert((ends.len()-1)*c.width.unwrap()+c.width.unwrap()==ends.len()*c.width.unwrap()) by(nonlinear_arith);}
    }
}

proof fn rewrite_key_bounds(c:RewritePlan,p:int)
    requires coordinates(c),key_byte(c,p),
    ensures 4<=p<c.len,p<(if c.width.is_some() {c.len as int} else {c.payload as int}),
{
    let i=choose|i:int|0<=i<c.count && #[trigger] key_at(c,i)<=p<key_at(c,i)+c.key;
    if c.width.is_some() {
        assert(4+i*c.stride+c.key<=c.len) by(nonlinear_arith)
            requires i<c.count,c.stride==c.key+c.width.unwrap(),c.len==4+c.count*c.key+c.count*c.width.unwrap(),c.key>0,c.width.unwrap()>=0;
    } else {assert(4+i*c.stride+c.key<=c.payload) by(nonlinear_arith)
        requires i<c.count,c.stride==c.key+4,c.payload==4+c.count*c.stride,c.key>0;}
}

/// Stored end words lie wholly before the payload, including the map terminal.
#[verifier::spinoff_prover]
proof fn rewrite_end_bounds(c:RewritePlan,i:int)
    requires coordinates(c),stored(c,i),0<=i<c.count,
    ensures 4<=4+i*c.stride+c.key,4+i*c.stride+c.key+4<=c.payload,
{
    if c.map {
        assert(4+i*c.stride+c.key+4<=4+c.count*c.stride) by(nonlinear_arith)
            requires i<c.count,c.stride==c.key+4,c.key>=0;
    } else {
        assert(4+i*c.stride+c.key+4<=4+(c.count-1)*4) by(nonlinear_arith)
            requires i+1<c.count,c.stride==4,c.key==0;
    }
}

proof fn rewrite_same_word(before:Seq<u8>,after:Seq<u8>,at:int)
    requires 0<=at,at+4<=before.len(),at+4<=after.len(),
        forall|p:int| at<=p<at+4 ==> after[p]==before[p],
    ensures words::le_word(before,at)==words::le_word(after,at),
{}

/// Checked prefix bounds contain the next loan in the final collection extent.
proof fn rewrite_slot_bounds(c:RewritePlan,done:int,end:int,len:int)
    requires coordinates(c),0<=done<c.count,0<=end<=c.total,0<=len<=c.total-end,
    ensures c.width.is_some() ==> 0<=(done+1)*c.key<=u32::MAX,
        4<=slot_start(c,done,end)<=slot_start(c,done,end)+len<=c.len,
{
    if c.width.is_some() {
        assert((done+1)*c.key<=c.count*c.key) by(nonlinear_arith)
            requires done<c.count,c.key>=0;
    }
}

/// Arbitrary child bytes cannot change completed ends, count or any key.
#[verifier::spinoff_prover]
proof fn rewrite_loan_frame(original:Seq<u8>,before:Seq<u8>,c:RewritePlan,ends:Seq<int>,done:int,end:int,len:int)
    requires coordinates(c),header_frame(original,before,c),prefix_ends(c,ends),directory(before,c,ends),
        ends.len()==done+1,end==ends[done],done<c.count,0<=len<=c.total-end,
        c.width.is_some() ==> len==c.width.unwrap(),words::le_word(original,0)==c.count,
    ensures forall|after:Seq<u8>| #[trigger] overwrite_frame(before,after,Span {
        start:slot_start(c,done,end) as u32,end:(slot_start(c,done,end)+len) as u32}) ==>
            header_frame(original,after,c) && directory(after,c,ends)
            && stream_shape(c,original,after,ends,done as usize,end as usize,Some(len as u32),true),
{
    let start=slot_start(c,done,end);
    if c.width.is_some() {assert((done+1)*c.key<=c.count*c.key) by(nonlinear_arith)
        requires done<c.count,c.key>=0;}
    assert(4<=start<=start+len<=c.len);
    assert forall|p:int| key_byte(c,p) implies p<start || start+len<=p by {
        let i=choose|i:int| 0<=i<c.count && #[trigger] key_at(c,i)<=p<key_at(c,i)+c.key;
        if c.width.is_some() {
            assert(end==done*c.width.unwrap());
            if i<=done {assert(4+i*c.stride+c.key<=start) by(nonlinear_arith)
                requires i<=done,c.stride==c.key+c.width.unwrap(),c.key>0,c.width.unwrap()>=0,start==4+done*c.width.unwrap()+(done+1)*c.key;}
            else {assert(start+len<=4+i*c.stride) by(nonlinear_arith)
                requires done<i,c.stride==c.key+c.width.unwrap(),c.key>0,len==c.width.unwrap(),start==4+done*c.width.unwrap()+(done+1)*c.key;}
        } else {assert(4+i*c.stride+c.key<=c.payload) by(nonlinear_arith)
            requires i<c.count,c.stride==c.key+4,c.key>0,c.payload==4+c.count*c.stride;}
    }
    assert forall|after:Seq<u8>| #[trigger] overwrite_frame(before,after,Span {start:start as u32,end:(start+len) as u32}) implies
        header_frame(original,after,c) && directory(after,c,ends)
            && stream_shape(c,original,after,ends,done as usize,end as usize,Some(len as u32),true) by {
        assert forall|p:int| 0<=p<4 || key_byte(c,p) implies after[p]==original[p] by {
            if key_byte(c,p) {rewrite_key_bounds(c,p);}
        }
        assert forall|i:int| 0<=i<ends.len()-1 && stored(c,i) implies
            #[trigger] words::le_word(after,4+i*c.stride+c.key)==ends[i+1] by {
            rewrite_end_bounds(c,i);
            assert forall|p:int| 4+i*c.stride+c.key<=p<4+i*c.stride+c.key+4 implies after[p]==before[p] by {}
            rewrite_same_word(before,after,4+i*c.stride+c.key);
        }
    }
}

/// Cumulative lengths partition the final payload without gaps or overlaps.
#[verifier::spinoff_prover]
proof fn rewrite_slots_partition(c: RewritePlan, ends: Seq<int>, i: int)
    requires coordinates(c), prefix_ends(c,ends), ends.len()==c.count+1,
        ends[c.count as int]==c.total, 0<=i<c.count,
    ensures 4<=slot_start(c,i,ends[i])<=slot_start(c,i,ends[i])+ends[i+1]-ends[i]<=c.len,
        i+1<c.count ==> slot_start(c,i,ends[i])+ends[i+1]-ends[i]
            + (if c.width.is_some() {c.key as int} else {0int}) == slot_start(c,i+1,ends[i+1]),
{
    if c.width.is_some() {
        assert((i+1)*c.key<=c.count*c.key) by(nonlinear_arith) requires i<c.count,c.key>=0;
        assert((i+1)*c.key+c.key==(i+2)*c.key) by(nonlinear_arith);
    }
}

/// The final directory selects exactly the slots issued by the stream.
#[verifier::spinoff_prover]
proof fn rewrite_projection(before: Seq<u8>, after: Seq<u8>, c: RewritePlan, ends: Seq<int>,layout:Layout,i:int)
    requires rewrite_result(before,after,c,ends), corresponds(c,layout),
        words::le_word(before,0)==c.count, 0<=i<c.count,
    ensures validation::child_span(after,layout,i) == (Span {
        start:slot_start(c,i,ends[i]) as u32,
        end:(slot_start(c,i,ends[i])+ends[i+1]-ends[i]) as u32 }),
{
    hide(words::le_word);
    reveal(coordinates);reveal(corresponds);reveal(rewrite_result);
    rewrite_same_word(before,after,0);
    assert(words::le_word(after,0)==c.count);
    rewrite_slots_partition(c,ends,i);
    match layout.head() {LayoutHead::List {..}=>{},_=>{}}
    if c.width.is_some() {
        assert(4+i*c.stride+c.key==slot_start(c,i,ends[i])) by(nonlinear_arith)
            requires c.stride==c.key+c.width.unwrap(),ends[i]==i*c.width.unwrap(),c.payload==4,
                slot_start(c,i,ends[i])==4+ends[i]+(i+1)*c.key;
        assert(ends[i+1]-ends[i]==c.width.unwrap()) by(nonlinear_arith)
            requires ends[i+1]==(i+1)*c.width.unwrap(),ends[i]==i*c.width.unwrap();
    } else {
        if i>0 { assert(stored(c,i-1));rewrite_end_bounds(c,i-1);assert(words::le_word(after,4+(i-1)*c.stride+c.key)==ends[i]); }
        if c.map || i+1<c.count { assert(stored(c,i));rewrite_end_bounds(c,i); assert(words::le_word(after,4+i*c.stride+c.key)==ends[i+1]); }
    }
    let desired=Span {start:slot_start(c,i,ends[i]) as u32,end:(slot_start(c,i,ends[i])+ends[i+1]-ends[i]) as u32};
    if c.map {
        assert(crate::map_layout::payload_start(after,layout)==c.payload);
        assert(crate::map_layout::value_span(after,layout,i as u32)==desired);
    } else {
        assert(crate::list_layout::element_width(layout)==c.width);
        if c.width.is_none() {
            assert(c.stride==4);
            assert(c.payload==4+(c.count as int-1)*c.stride);
            assert(c.payload==4+4*(c.count as int-1)) by(nonlinear_arith)
                requires c.payload==4+(c.count as int-1)*c.stride,c.stride==4;
            assert(crate::struct_layout::directory_size(words::le_word(after,0))==4*(c.count-1));
        }
        assert(crate::list_layout::payload_start(after,layout)==c.payload);
        assert(crate::list_layout::element_span(after,layout,i as u32)==desired);
    }
}

/// Streamed ends establish the exact indexed or fixed list partition.
#[verifier::spinoff_prover]
proof fn rewrite_list_shape(after:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout)
    requires coordinates(c),corresponds(c,layout),!c.map,after.len()==c.len,
        words::le_word(after,0)==c.count,prefix_ends(c,ends),ends.len()==c.count+1,
        ends[c.count as int]==c.total,directory(after,c,ends),
    ensures crate::list_layout::list_shape(after,layout),
{
    hide(words::le_word);
    assert(crate::list_layout::element_width(layout)==c.width);
    if c.width.is_none() {
        if c.count>0 {assert(c.stride==4 && c.key==0);}
        assert forall|i:int|0<=i<(c.count as int)-1 implies {
            let word=#[trigger] words::le_word(after,4+4*i);
            &&& 0<=word<=c.total
            &&& i>0 ==> words::le_word(after,4+4*(i-1))<=word
        } by {
            rewrite_end_bounds(c,i);
            assert(words::le_word(after,4+i*c.stride+c.key)==ends[i+1]);
            if i>0 {rewrite_end_bounds(c,i-1);assert(stored(c,i-1));
                assert(words::le_word(after,4+(i-1)*c.stride+c.key)==ends[i]);}
        }
    }
    assert(crate::list_layout::payload_start(after,layout)==c.payload);
}

/// Indexed maps store every cumulative end, including the exact terminal.
#[verifier::spinoff_prover]
proof fn rewrite_map_shape(after:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout)
    requires coordinates(c),corresponds(c,layout),c.map,after.len()==c.len,
        words::le_word(after,0)==c.count,prefix_ends(c,ends),ends.len()==c.count+1,
        ends[c.count as int]==c.total,directory(after,c,ends),
    ensures crate::map_layout::map_shape(after,layout),
{
    hide(words::le_word);
    reveal(coordinates);reveal(corresponds);
    assert(matches!(layout.head(),LayoutHead::Map {..}));
    assert(crate::map_layout::value_width(layout)==c.width);
    let stride=crate::map_layout::entry_stride(layout);
    let key=crate::map_layout::key_width(layout);
    assert(key==c.key);
    if c.width.is_none() {
        if c.count>0 {assert(stride==c.stride);}else {
            assert(c.count==0);assert(c.total==0);
            assert(c.width.is_none() && !c.width.is_some() && c.map);
            assert(c.payload==4+(c.count as int)*c.stride);
            assert(c.payload==4);assert(c.len==4);assert(after.len()==4);
        }
        assert(crate::map_layout::payload_start(after,layout)==c.payload);
        assert forall|i:int|0<=i<c.count implies {
            let end=#[trigger] words::le_word(after,4+i*stride+key);
            &&& 0<=end<=c.total
            &&& i>0 ==> words::le_word(after,4+(i-1)*stride+key)<=end
        } by {
            assert(stride==c.stride);rewrite_end_bounds(c,i);
            assert(words::le_word(after,4+i*stride+key)==ends[i+1]);
            if i>0 {assert(stored(c,i-1));rewrite_end_bounds(c,i-1);
                assert(words::le_word(after,4+(i-1)*stride+key)==ends[i]);}
        }
        if c.count>0 {assert(words::le_word(after,4+(c.count as int-1)*stride+key)==c.total);}
    } else {
        assert(after.len()==4+c.count*stride) by(nonlinear_arith)
            requires after.len()==c.len,c.len==4+c.count*c.key+c.total,
                c.total==c.count*c.width.unwrap(),stride==c.key+c.width.unwrap();
    }
}

/// Every immutable key retains its exact byte encoding and ordering evidence.
#[verifier::spinoff_prover]
proof fn rewrite_keys_frame(before:Seq<u8>,after:Seq<u8>,c:RewritePlan,layout:Layout)
    requires coordinates(c),corresponds(c,layout),c.map,header_frame(before,after,c),
    ensures crate::search::keys_ordered(before,layout,c.count as int)==crate::search::keys_ordered(after,layout,c.count as int),
        crate::search::keys_canonical(before,layout,c.count as int)==crate::search::keys_canonical(after,layout,c.count as int),
{
    hide(crate::search::keys_ordered);hide(crate::search::keys_canonical);
    if c.count>0 {assert(c.stride==crate::map_layout::entry_stride(layout));}
    assert forall|i:int|0<=i<c.count implies
        #[trigger] crate::search::encoded_key(after,layout,i)==crate::search::encoded_key(before,layout,i) by {
        if c.width.is_some() {assert(4+i*c.stride+c.key<=c.len) by(nonlinear_arith)
            requires i<c.count,c.stride==c.key+c.width.unwrap(),c.len==4+c.count*c.key+c.count*c.width.unwrap(),c.key>0,c.width.unwrap()>=0;}
        else {assert(4+i*c.stride+c.key<=c.payload) by(nonlinear_arith)
            requires i<c.count,c.stride==c.key+4,c.payload==4+c.count*c.stride,c.key>0;}
        assert(after.subrange(4+i*c.stride,4+i*c.stride+c.key) =~= before.subrange(4+i*c.stride,4+i*c.stride+c.key)) by {
            assert forall|p:int| #[trigger] key_at(c,i)<=p<key_at(c,i)+c.key implies after[p]==before[p] by {
                assert(key_byte(c,p));rewrite_key_bounds(c,p);
            }
        }
    }
    nested::keys_frame(before,after,layout,c.count as int);
}

/// Count and key frames plus streamed ends establish local collection validity.
#[verifier::spinoff_prover]
proof fn rewrite_local_valid(before:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout)
    requires rewrite_result(before,after,c,ends),corresponds(c,layout),
        words::le_word(before,0)==c.count,validation::local_valid(before,layout,true),
    ensures validation::local_valid(after,layout,true),
{
    hide(crate::search::keys_ordered);hide(crate::search::keys_canonical);
    rewrite_same_word(before,after,0);
    assert(words::le_word(after,0)==c.count);
    if c.map {rewrite_map_shape(after,c,ends,layout);rewrite_keys_frame(before,after,c,layout);}
    else {rewrite_list_shape(after,c,ends,layout);}
}

proof fn rewrite_child_domain(bytes:Seq<u8>,layout:Layout,i:int,budget:nat)
    ensures validation::child_valid(bytes,layout,i,budget,true)==validation::schema_valid(
        bytes.subrange(validation::child_span(bytes,layout,i).start as int,validation::child_span(bytes,layout,i).end as int),
        validation::child_layout(layout,i),budget,true),
{}

/// Individual encoded-child validity suffices; completed validity is not assumed.
#[verifier::spinoff_prover]
proof fn rewrite_selected_preserves(before:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout,budget:nat)
    requires rewrite_result(before,after,c,ends), corresponds(c,layout),
        words::le_word(before,0)==c.count, validation::local_valid(before,layout,true), budget>0,
        forall|i:int| 0<=i<c.count ==> #[trigger] validation::schema_valid(
            after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
            validation::child_layout(layout,i),(budget-1) as nat,true),
    ensures validation::schema_valid(after,layout,budget,true),
{
    hide(validation::schema_valid);
    rewrite_local_valid(before,after,c,ends,layout);
    assert(words::le_word(after,0)==c.count);
    assert forall|i:int| 0<=i<c.count implies
        #[trigger] validation::child_valid(after,layout,i,(budget-1) as nat,true) by {
        rewrite_projection(before,after,c,ends,layout,i);
        rewrite_slots_partition(c,ends,i);
        let span=validation::child_span(after,layout,i);
        assert(span.start as int==slot_start(c,i,ends[i]));
        assert(span.end as int==slot_start(c,i,ends[i])+ends[i+1]-ends[i]);
        assert(validation::schema_valid(after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
            validation::child_layout(layout,i),(budget-1) as nat,true));
        rewrite_child_domain(after,layout,i,(budget-1) as nat);
    }
    crate::preservation::children_from_all(after,layout,0,(budget-1) as nat,true);
    reveal_with_fuel(validation::schema_valid,1);
}

/// The enclosing generic splice lifts exact rewritten slots to a valid root.
#[verifier::spinoff_prover]
proof fn rewrite_publish(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,plan:Plan,c:RewritePlan,ends:Seq<int>)
    requires replacement_checked(before,plan),subtree_plan(plan),replacement_result(before,encoded,after,plan),
        validation::schema_valid(before.subrange(12,plan.old_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        rewrite_result(before.subrange(plan.region.start as int,plan.region.end as int),
            encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),c,ends),
        corresponds(c,plan.trail.selected),
        forall|i:int| 0<=i<c.count ==> #[trigger] validation::schema_valid(
            encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
            validation::child_layout(plan.trail.selected,i),(MAX_DEPTH-plan.trail.depth-1) as nat,true),
    ensures validation::schema_valid(after.subrange(12,plan.new_used as int),plan.trail.root,MAX_DEPTH as nat,true),
{
    nested::trail_schema(before,plan,MAX_DEPTH as nat,true,plan.trail.depth as int);
    let old=before.subrange(plan.region.start as int,plan.region.end as int);
    let new=encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len);
    assert(words::le_word(old,0)==c.count);
    assert forall|i:int| 0<=i<c.count implies #[trigger] validation::schema_valid(
        new.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(plan.trail.selected,i),(MAX_DEPTH-plan.trail.depth-1) as nat,true) by {
        rewrite_slots_partition(c,ends,i);
        assert(new.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]) =~=
            encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]));
    }
    rewrite_selected_preserves(old,new,c,ends,plan.trail.selected,(MAX_DEPTH-plan.trail.depth) as nat);
    subtree_replace_preserves(before,encoded,after,plan);
}

// Typed encode_slot composition: each successful exact loan contributes one
// child's semantic validity, while its full-buffer frame retains the prefix.
spec fn encoded_prefix(bytes:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout,budget:nat,done:int)->bool {
    forall|i:int| 0<=i<done ==> #[trigger] validation::schema_valid(
        bytes.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(layout,i),budget,true)
}

#[verifier::spinoff_prover]
proof fn rewrite_prefix_order(c:RewritePlan,ends:Seq<int>,i:int,j:int)
    requires prefix_ends(c,ends),0<=i<=j<ends.len(),
    ensures ends[i]<=ends[j],
    decreases j-i,
{
    if i<j {rewrite_prefix_order(c,ends,i,j-1);}
}

#[verifier::spinoff_prover]
proof fn rewrite_encode_slot(before:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout,budget:nat,done:int,slot:Span)
    requires before.len()==c.len, coordinates(c),prefix_ends(c,ends),ends.len()==c.count+1,ends[c.count as int]==c.total,0<=done<c.count,
        encoded_prefix(before,c,ends,layout,budget,done),
        slot.start as int==slot_start(c,done,ends[done]),
        slot.end as int==slot_start(c,done,ends[done])+ends[done+1]-ends[done],
        slot.start<=slot.end<=before.len(),overwrite_frame(before,after,slot),
        validation::schema_valid(after.subrange(slot_start(c,done,ends[done]),
            slot_start(c,done,ends[done])+ends[done+1]-ends[done]),validation::child_layout(layout,done),budget,true),
    ensures encoded_prefix(after,c,ends,layout,budget,done+1),
{
    hide(validation::schema_valid);
    reveal(encoded_prefix);
    rewrite_slots_partition(c,ends,done);
    assert forall|i:int| 0<=i<done+1 implies #[trigger] validation::schema_valid(
        after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(layout,i),budget,true) by {
        if i<done {
            rewrite_slots_partition(c,ends,i);
            rewrite_prefix_order(c,ends,i+1,done);
            assert(slot_start(c,i,ends[i])+ends[i+1]-ends[i]<=slot_start(c,done,ends[done])) by(nonlinear_arith)
                requires i<done,ends[i+1]<=ends[done],c.key>=0,
                    slot_start(c,i,ends[i])==c.payload+ends[i]+if c.width.is_some(){(i+1)*c.key}else{0},
                    slot_start(c,done,ends[done])==c.payload+ends[done]+if c.width.is_some(){(done+1)*c.key}else{0};
            assert(after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]) =~=
                before.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]));
        }
    }
}

// Keyless preflight supplies value lengths only. The old map supplies all key
// coordinates; no input key identity is a premise of typed slot composition.
#[verifier::spinoff_prover]
proof fn rewrite_measured_order(lengths:Seq<u32>,ends:Seq<int>,i:int,j:int)
    requires ends.len()==lengths.len()+1,0<=i<=j<ends.len(),
        forall|k:int|0<=k<lengths.len() ==> ends[k+1]==ends[k]+#[trigger] lengths[k],
    ensures ends[i]<=ends[j],
    decreases j-i,
{
    if i<j {
        rewrite_measured_order(lengths,ends,i,j-1);
        assert(0<=lengths[j-1]);
        assert(ends[j]==ends[j-1]+lengths[j-1]);
    }
}

#[verifier::spinoff_prover]
proof fn rewrite_keyless_preflight(c:RewritePlan,lengths:Seq<u32>,ends:Seq<int>)
    requires coordinates(c),c.map,lengths.len()==c.count,
        ends.len()==lengths.len()+1,ends[0]==0,ends[c.count as int]==c.total,
        forall|i:int|0<=i<lengths.len() ==> #[trigger] ends[i+1]==ends[i]+lengths[i],
        c.width.is_some() ==> forall|i:int|0<=i<ends.len() ==> #[trigger] ends[i]==i*c.width.unwrap(),
    ensures prefix_ends(c,ends),ends.len()==c.count+1,ends[c.count as int]==c.total,
{
    assert forall|i:int|0<=i<ends.len() implies 0<=#[trigger] ends[i]<=c.total by {
        rewrite_measured_order(lengths,ends,0,i);
        rewrite_measured_order(lengths,ends,i,c.count as int);
    }
    assert forall|i:int|0<=i<ends.len()-1 implies #[trigger] ends[i]<=ends[i+1] by {
        rewrite_measured_order(lengths,ends,i,i+1);
    }
}

/// Publishing an end word cannot invalidate already encoded payload slots.
#[verifier::spinoff_prover]
proof fn rewrite_encode_advance(before:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>,layout:Layout,budget:nat,done:int,word:Span)
    requires before.len()==c.len,coordinates(c),prefix_ends(c,ends),ends.len()==c.count+1,
        ends[c.count as int]==c.total,0<=done<=c.count,
        encoded_prefix(before,c,ends,layout,budget,done),
        if c.width.is_some() {after==before} else {
            word.start<=word.end<=c.payload && overwrite_frame(before,after,word)},
    ensures encoded_prefix(after,c,ends,layout,budget,done),
{
    hide(validation::schema_valid);
    reveal(encoded_prefix);
    if c.width.is_none() {
        assert forall|i:int| 0<=i<done implies #[trigger] validation::schema_valid(
            after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
            validation::child_layout(layout,i),budget,true) by {
            rewrite_slots_partition(c,ends,i);
            assert(c.payload<=slot_start(c,i,ends[i]));
            assert(after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]) =~=
                before.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]));
        }
    }
}

/// Completed typed encoders compose with the whole-root publication theorem.
#[verifier::spinoff_prover]
proof fn rewrite_typed_publish(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,plan:Plan,c:RewritePlan,ends:Seq<int>)
    requires replacement_checked(before,plan),subtree_plan(plan),replacement_result(before,encoded,after,plan),
        validation::schema_valid(before.subrange(12,plan.old_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        rewrite_result(before.subrange(plan.region.start as int,plan.region.end as int),
            encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),c,ends),
        corresponds(c,plan.trail.selected),
        encoded_prefix(encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),
            c,ends,plan.trail.selected,(MAX_DEPTH-plan.trail.depth-1) as nat,c.count as int),
    ensures validation::schema_valid(after.subrange(12,plan.new_used as int),plan.trail.root,MAX_DEPTH as nat,true),
{
    reveal(encoded_prefix);
    assert forall|i:int| 0<=i<c.count implies #[trigger] validation::schema_valid(
        encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(plan.trail.selected,i),(MAX_DEPTH-plan.trail.depth-1) as nat,true) by {
        rewrite_slots_partition(c,ends,i);
        assert(encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len).subrange(
            slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]) =~=
            encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]));
    }
    rewrite_publish(before,encoded,after,plan,c,ends);
}

/// The retained payload base equals the format's count-dependent payload base.
#[verifier::spinoff_prover]
proof fn rewrite_payload(bytes:Seq<u8>,layout:Layout,c:RewritePlan)
    requires coordinates(c),corresponds(c,layout),4<=bytes.len(),words::le_word(bytes,0)==c.count,
    ensures if c.map {crate::map_layout::payload_start(bytes,layout)==c.payload}
        else {crate::list_layout::payload_start(bytes,layout)==c.payload},
{
    hide(words::le_word);
    reveal(coordinates);reveal(corresponds);
    if c.map {
        assert(crate::map_layout::value_width(layout)==c.width);
        if c.width.is_none() {
            if c.count>0 {assert(crate::map_layout::entry_stride(layout)==c.stride);}
            else {assert(c.count==0);assert(c.payload==4);}
        }
    } else {
        assert(crate::list_layout::element_width(layout)==c.width);
        if c.width.is_none() {
            if c.count>0 {
                assert(c.stride==4);
                assert(c.payload==4+4*(c.count as int-1)) by(nonlinear_arith)
                    requires c.payload==4+(c.count as int-1)*c.stride,c.stride==4;
                assert(crate::struct_layout::directory_size(words::le_word(bytes,0))==4*(c.count as int-1));
            } else {assert(c.count==0);assert(c.payload==4);}
        }
    }
}

/// Detached rewrite coordinates request precisely the ordinary subtree geometry.
#[verifier::spinoff_prover]
proof fn rewrite_admission_matches_generic(before:Seq<u8>,plan:Plan,c:RewritePlan,initial:Seq<u8>)
    requires replacement_checked(before,plan),subtree_plan(plan),plan.required<=before.len(),coordinates(c),
        corresponds(c,plan.trail.selected),plan.replacement_len==c.len,
        validation::local_valid(before.subrange(plan.region.start as int,plan.region.end as int),plan.trail.selected,true),
        words::le_word(before,plan.region.start as int)==c.count,
        initial==before.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),
    ensures plan.new_used==plan.old_used+c.len-(plan.region.end-plan.region.start),
        plan.required==if plan.new_used>plan.old_used {plan.new_used} else {plan.old_used},
        plan.delta==c.len-(plan.region.end-plan.region.start),
        header_frame(before.subrange(plan.region.start as int,plan.region.end as int),initial,c),
        words::le_word(initial,0)==c.count,
{
    hide(words::le_word);
    reveal(subtree_plan);reveal(target);reveal(coordinates);reveal(corresponds);
    let old=before.subrange(plan.region.start as int,plan.region.end as int);
    indexed_insert::word_in_slice(before,plan.region.start as int,plan.region.end as int,0);
    indexed_insert::word_in_slice(before,plan.region.start as int,plan.region.start+plan.replacement_len,0);
    assert(words::le_word(old,0)==c.count);
    rewrite_payload(old,plan.trail.selected,c);
    if c.map {
        assert(crate::map_layout::value_width(plan.trail.selected)==c.width);
        if c.count>0 {assert(crate::map_layout::entry_stride(plan.trail.selected)==c.stride);}
        if c.width.is_some() {
            assert(old.len()==c.len) by(nonlinear_arith)
                requires old.len()==4+c.count*crate::map_layout::entry_stride(plan.trail.selected),
                    crate::map_layout::entry_stride(plan.trail.selected)==c.key+c.width.unwrap(),
                    c.len==4+c.count*c.key+c.count*c.width.unwrap();
        } else {assert(crate::map_layout::payload_start(old,plan.trail.selected)==c.payload);}
    } else {
        assert(crate::list_layout::element_width(plan.trail.selected)==c.width);
        if c.width.is_some() {assert(old.len()==c.len);}
        else {
            if c.count>0 {assert(c.stride==4);}
            assert(crate::list_layout::payload_start(old,plan.trail.selected)==c.payload);
        }
    }
    assert(initial.len()==c.len);
    assert((if c.width.is_some() {c.len as int}else {c.payload as int})<=old.len());
    assert forall|p:int|0<=p<4 || key_byte(c,p) implies #[trigger] initial[p]==old[p] by {
        if key_byte(c,p) {rewrite_key_bounds(c,p);}
        assert(0<=p<initial.len() && p<old.len());
        assert(initial[p]==before[plan.region.start+p]);
        assert(old[p]==before[plan.region.start+p]);
    }
}

/// The incoming key/count frame composes with the completed stream's frame.
#[verifier::spinoff_prover]
proof fn rewrite_admission_transport(old:Seq<u8>,initial:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>)
    requires coordinates(c),header_frame(old,initial,c),rewrite_result(initial,after,c,ends),
    ensures rewrite_result(old,after,c,ends),
{
    hide(header_frame);
    assert(header_frame(old,initial,c));
    reveal(header_frame);
    rewrite_same_word(old,initial,0);
    assert forall|p:int|0<=p<4 || key_byte(c,p) implies #[trigger] after[p]==old[p] by {
        if key_byte(c,p) {rewrite_key_bounds(c,p);}
    }
}


/// Compact direct-tail coordinates admit the checked general subtree plan.
#[verifier::spinoff_prover]
proof fn rewrite_tail_plan(t:RewriteTail,w:StringWrite,before:Seq<u8>,input:u32)
    ->(plan:Plan<'static>)
    requires tail_checked(t,before),tail_write(t,w,input),
        validation::schema_valid(before.subrange(12,t.used as int),t.root@,MAX_DEPTH as nat,true),
    ensures replacement_checked(before,plan),subtree_plan(plan),
        plan.trail.root==t.root@ && plan.trail.selected==t.selected && plan.trail.depth==1,
        plan.region==(Span{start:t.start,end:t.used}),plan.old_used==t.used,
        plan.replacement_len==input && plan.new_used==t.start+input,
        forall|i:int|0<=i<plan.trail.depth ==> #[trigger] plan.trail.nodes[i].run.count==0,
        admission::subtree_ready(before,plan.trail,input,before.len() as u32),
        plan==admission::subtree_plan_for(before,t.path@,plan.trail,input,before.len() as u32),
{
    let s=t.selection@;
    admission::direct_checked(before,t.root@,t.path@,t.used,s);
    let trail=admission::direct_trail(before,t.root@,t.path@,t.used,s);
    let delta=(input as int-(s.span.end-s.span.start)) as i64;
    admission::idle_runs(before,trail,t.path@,t.used,delta);
    assert(admission::subtree_ready(before,trail,input,before.len() as u32));
    admission::subtree_checked(before,t.path@,trail,input,before.len() as u32);
    let plan=admission::subtree_plan_for(before,t.path@,trail,input,before.len() as u32);
    plan
}

/// The compact publication frame equals the ordinary subtree result.
#[verifier::spinoff_prover]
proof fn rewrite_tail_matches_generic(t:RewriteTail,w:StringWrite,before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,input:u32)
    ->(plan:Plan<'static>)
    requires tail_checked(t,before),tail_write(t,w,input),
        validation::schema_valid(before.subrange(12,t.used as int),t.root@,MAX_DEPTH as nat,true),
        string_result(before,encoded,after,t.start,(t.start+input) as u32,t.used,(t.start+input) as u32),
    ensures replacement_checked(before,plan),subtree_plan(plan),replacement_result(before,encoded,after,plan),
        plan.trail.root==t.root@ && plan.trail.selected==t.selected && plan.trail.depth==1,
        plan.region==(Span{start:t.start,end:t.used}),plan.old_used==t.used,
        plan.replacement_len==input && plan.new_used==t.start+input,
{
    let plan=rewrite_tail_plan(t,w,before,input);
    string_matches_generic(before,encoded,after,plan,t.start,(t.start+input) as u32,t.used,(t.start+input) as u32);
    plan
}

/// The exact incoming loan retains the checked source's count and every key.
#[verifier::spinoff_prover]
proof fn rewrite_tail_admission(t:RewriteTail,w:StringWrite,before:Seq<u8>,initial:Seq<u8>,c:RewritePlan)
    requires tail_checked(t,before),tail_write(t,w,c.len),coordinates(c),corresponds(c,t.selected),
        validation::schema_valid(before.subrange(12,t.used as int),t.root@,MAX_DEPTH as nat,true),
        validation::local_valid(before.subrange(t.start as int,t.used as int),t.selected,true),
        words::le_word(before,t.start as int)==c.count,
        initial==before.subrange(t.start as int,t.start+c.len),
    ensures header_frame(before.subrange(t.start as int,t.used as int),initial,c),words::le_word(initial,0)==c.count,
{
    let plan=rewrite_tail_plan(t,w,before,c.len);
    rewrite_admission_matches_generic(before,plan,c,initial);
}

/// Individual encoded children refine compact tail publication to a valid root.
#[verifier::spinoff_prover]
proof fn rewrite_tail_publish(t:RewriteTail,w:StringWrite,before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,c:RewritePlan,ends:Seq<int>)
    requires tail_checked(t,before),tail_write(t,w,c.len),
        string_result(before,encoded,after,t.start,(t.start+c.len) as u32,t.used,(t.start+c.len) as u32),
        validation::schema_valid(before.subrange(12,t.used as int),t.root@,MAX_DEPTH as nat,true),
        rewrite_result(before.subrange(t.start as int,t.used as int),encoded.subrange(t.start as int,t.start+c.len),c,ends),
        corresponds(c,t.selected),
        forall|i:int|0<=i<c.count ==> #[trigger] validation::schema_valid(
            encoded.subrange(t.start+slot_start(c,i,ends[i]),t.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
            validation::child_layout(t.selected,i),(MAX_DEPTH-2) as nat,true),
    ensures validation::schema_valid(after.subrange(12,t.start+c.len),t.root@,MAX_DEPTH as nat,true),
{
    let plan=rewrite_tail_matches_generic(t,w,before,encoded,after,c.len);
    assert forall|i:int|0<=i<c.count implies #[trigger] validation::schema_valid(
        encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(plan.trail.selected,i),(MAX_DEPTH-plan.trail.depth-1) as nat,true) by {
        rewrite_slots_partition(c,ends,i);
        assert(t.start+c.len<=encoded.len());
        assert(0<=t.start+slot_start(c,i,ends[i])<=t.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]<=encoded.len());
        assert(validation::schema_valid(
            encoded.subrange(t.start+slot_start(c,i,ends[i]),t.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
            validation::child_layout(t.selected,i),(MAX_DEPTH-2) as nat,true));
        assert(plan.region.start==t.start && plan.trail.selected==t.selected && plan.trail.depth==1);
    }
    rewrite_publish(before,encoded,after,plan,c,ends);
}

/// Concrete nonempty children inhabit compact admission and publication.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn rewrite_tail_witness()
    ensures true,
    no_unwind
{
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid,6);
        reveal_with_fuel(crate::layout::descriptor_fields,5);
        reveal_with_fuel(validation::schema_valid,5);
        reveal_with_fuel(validation::children_valid,5);
        reveal_with_fuel(crate::utf8::utf8_valid,4);
    }
    let mut raw=[1u8,0,0,0,0,0,0,0,22,0,0,0,2,0,0,0,1,0,0,0,97,98];
    let ghost before=raw@;
    let ghost list=witnesses::INDEXED_LIST;
    let ghost root=Layout::Struct {fixed_bytes:0,fixed:vstd::array::spec_array_as_slice(&[]),
        variable:vstd::array::spec_array_as_slice(&[crate::VariableField{declaration:0,layout:&witnesses::INDEXED_LIST}])};
    let ghost s=Selection {payload_len:10,span:Span{start:0,end:10},layout:list,ordinal:0,
        run:PatchRun{offset:0,stride:4,count:0}};
    let t=RewriteTail{start:12,used:22,selected:witnesses::INDEXED_LIST,original:Ghost(before),
        root:Ghost(root),path:Ghost(witnesses::field_path()),selection:Ghost(s)};
    let w=string_edit::direct_write(&mut raw,12,22,22);
    proof {
        let after=before.update(16,0).update(20,99).update(21,100);
        let c=RewritePlan {count:2,payload:8,total:2,len:10,key:0,stride:4,width:None,map:false};
        let ends=seq![0int,0,2];
        crate::utf8::ascii_valid(seq![97u8]);crate::utf8::ascii_valid(seq![98u8]);crate::utf8::ascii_valid(seq![99u8,100]);
        assert(before.subrange(12,22)==seq![2u8,0,0,0,1,0,0,0,97,98]);
        assert(after.subrange(12,22)==seq![2u8,0,0,0,0,0,0,0,99,100]);
        assert(tail_checked(t,before));
        assert(string_result(before,after,after,12,22,22,22));
        assert(validation::schema_valid(before.subrange(12,22),list,(MAX_DEPTH-1) as nat,true));
        assert(validation::child_layout(root,0)==list);
        assert(validation::child_span(before.subrange(12,22),root,0)==(Span{start:0,end:10}));
        assert(before.subrange(12,22).subrange(0,10) =~= before.subrange(12,22));
        assert(validation::child_valid(before.subrange(12,22),root,0,(MAX_DEPTH-1) as nat,true));
        assert(validation::children_valid(before.subrange(12,22),root,0,(MAX_DEPTH-1) as nat,true));
        assert(validation::schema_valid(before.subrange(12,22),root,MAX_DEPTH as nat,true));
        rewrite_tail_admission(t,w,before,before.subrange(12,22),c);
        rewrite_tail_matches_generic(t,w,before,after,after,10);
        rewrite_tail_publish(t,w,before,after,after,c,ends);
    }
}

/// Nonempty list and map witnesses include mixed empty/growing exact slots.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rewrite_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal_with_fuel(crate::utf8::utf8_valid,4);
    let list=witnesses::INDEXED_LIST;
    let old=seq![2u8,0,0,0,1,0,0,0,97,98];
    let new=seq![2u8,0,0,0,0,0,0,0,99,100];
    let c=RewritePlan {count:2,payload:8,total:2,len:10,key:0,stride:4,width:None,map:false};
    let ends=seq![0int,0,2];
    crate::utf8::ascii_valid(seq![97u8]);crate::utf8::ascii_valid(seq![98u8]);crate::utf8::ascii_valid(seq![99u8,100]);
    rewrite_prefix_append(c,seq![0int],2);
    rewrite_loan_frame(old,old,c,seq![0int],0,0,2);
    rewrite_local_valid(old,new,c,ends,list);
    rewrite_slots_partition(c,ends,0);
    rewrite_selected_preserves(old,new,c,ends,list,MAX_DEPTH as nat);
    let before=witnesses::account(old);
    let after=witnesses::account(new);
    let plan=witnesses::replacement(before,list,10);
    assert(before.subrange(12,22) =~= old);
    assert(plan.region.start==12 && plan.replacement_len==10);
    rewrite_admission_matches_generic(before,plan,c,old);
    rewrite_admission_transport(old,old,new,c,ends);
    rewrite_publish(before,after,after,plan,c,ends);
}

/// A nonempty map witnesses preserved keys, empty values and complete publication.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rewrite_map_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal_with_fuel(crate::utf8::utf8_valid,4);
    let ends=seq![0int,0,2];
    crate::utf8::ascii_valid(seq![97u8]);crate::utf8::ascii_valid(seq![98u8]);crate::utf8::ascii_valid(seq![99u8,100]);
    let map=witnesses::INDEXED_MAP;
    let old_map=seq![2u8,0,0,0,1,1,0,0,0,3,2,0,0,0,97,98];
    let new_map=seq![2u8,0,0,0,1,0,0,0,0,3,2,0,0,0,99,100];
    assert(crate::search::same_prefix(seq![1u8],seq![3u8],0));
    assert(crate::search::lex_less(seq![1u8],seq![3u8]));
    assert(crate::search::encoded_key(old_map,map,0)==seq![1u8]);
    assert(crate::search::encoded_key(old_map,map,1)==seq![3u8]);
    assert(validation::local_valid(old_map,map,true));
    let m=RewritePlan {count:2,payload:14,total:2,len:16,key:1,stride:5,width:None,map:true};
    rewrite_slots_partition(m,ends,1);
    rewrite_selected_preserves(old_map,new_map,m,ends,map,MAX_DEPTH as nat);
    let before_map=witnesses::account(old_map);
    let after_map=witnesses::account(new_map);
    let map_plan=witnesses::replacement(before_map,map,16);
    assert(before_map.subrange(12,28) =~= old_map);
    assert(map_plan.region.start==12 && map_plan.replacement_len==16);
    rewrite_admission_matches_generic(before_map,map_plan,m,old_map);
    assert(before_map.subrange(12,28) =~= old_map);
    assert(validation::schema_valid(old_map,map,MAX_DEPTH as nat,true));
    rewrite_publish(before_map,after_map,after_map,map_plan,m,ends);
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rewrite_typed_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal_with_fuel(crate::utf8::utf8_valid,4);
    let list=witnesses::INDEXED_LIST;
    let old=seq![2u8,0,0,0,1,0,0,0,97,98];
    let new=seq![2u8,0,0,0,0,0,0,0,99,100];
    let c=RewritePlan {count:2,payload:8,total:2,len:10,key:0,stride:4,width:None,map:false};
    let ends=seq![0int,0,2];
    crate::utf8::ascii_valid(seq![97u8]);crate::utf8::ascii_valid(seq![98u8]);crate::utf8::ascii_valid(seq![99u8,100]);
    rewrite_prefix_append(c,seq![0int],2);
    rewrite_loan_frame(old,old,c,seq![0int],0,0,2);
    rewrite_local_valid(old,new,c,ends,list);
    rewrite_slots_partition(c,ends,0);
    rewrite_selected_preserves(old,new,c,ends,list,MAX_DEPTH as nat);
    let before=witnesses::account(old);
    let after=witnesses::account(new);
    let plan=witnesses::replacement(before,list,10);
    assert(before.subrange(12,22) =~= old);
    assert(plan.region.start==12 && plan.replacement_len==10);
    rewrite_admission_matches_generic(before,plan,c,old);
    rewrite_admission_transport(old,old,new,c,ends);
    let mid=new.update(8,97u8).update(9,98u8);
    rewrite_encode_slot(old,old,c,ends,list,(MAX_DEPTH-1) as nat,0,Span{start:8,end:8});
    rewrite_encode_advance(old,mid,c,ends,list,(MAX_DEPTH-1) as nat,1,Span{start:4,end:8});
    rewrite_encode_slot(mid,new,c,ends,list,(MAX_DEPTH-1) as nat,1,Span{start:8,end:10});
    rewrite_typed_publish(before,after,after,plan,c,ends);
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rewrite_typed_map_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal_with_fuel(crate::utf8::utf8_valid,4);
    let ends=seq![0int,0,2];
    crate::utf8::ascii_valid(seq![97u8]);crate::utf8::ascii_valid(seq![98u8]);crate::utf8::ascii_valid(seq![99u8,100]);
    let map=witnesses::INDEXED_MAP;
    let old_map=seq![2u8,0,0,0,1,1,0,0,0,3,2,0,0,0,97,98];
    let new_map=seq![2u8,0,0,0,1,0,0,0,0,3,2,0,0,0,99,100];
    assert(crate::search::same_prefix(seq![1u8],seq![3u8],0));
    assert(crate::search::lex_less(seq![1u8],seq![3u8]));
    assert(crate::search::encoded_key(old_map,map,0)==seq![1u8]);
    assert(crate::search::encoded_key(old_map,map,1)==seq![3u8]);
    assert(validation::local_valid(old_map,map,true));
    let m=RewritePlan {count:2,payload:14,total:2,len:16,key:1,stride:5,width:None,map:true};
    rewrite_keyless_preflight(m,seq![0u32,2],ends);
    rewrite_slots_partition(m,ends,1);
    rewrite_selected_preserves(old_map,new_map,m,ends,map,MAX_DEPTH as nat);
    let before_map=witnesses::account(old_map);
    let after_map=witnesses::account(new_map);
    let map_plan=witnesses::replacement(before_map,map,16);
    assert(before_map.subrange(12,28) =~= old_map);
    assert(map_plan.region.start==12 && map_plan.replacement_len==16);
    rewrite_admission_matches_generic(before_map,map_plan,m,old_map);
    assert(before_map.subrange(12,28) =~= old_map);
    assert(validation::schema_valid(old_map,map,MAX_DEPTH as nat,true));
    let mid=new_map.update(14,97u8).update(15,98u8);
    rewrite_encode_slot(old_map,old_map,m,ends,map,(MAX_DEPTH-1) as nat,0,Span{start:14,end:14});
    rewrite_encode_advance(old_map,mid,m,ends,map,(MAX_DEPTH-1) as nat,1,Span{start:5,end:9});
    rewrite_encode_slot(mid,new_map,m,ends,map,(MAX_DEPTH-1) as nat,1,Span{start:14,end:16});
    rewrite_typed_publish(before_map,after_map,after_map,map_plan,m,ends);
}

/// Fixed value loans establish the same conditional collection preservation.
#[verifier::spinoff_prover]
proof fn rewrite_fixed_fill_preserves(before:Seq<u8>,after:Seq<u8>,c:RewritePlan,layout:Layout,budget:nat)
    requires coordinates(c),corresponds(c,layout),c.width.is_some(),c.width.unwrap()>0,
        before.len()==c.len,after.len()==c.len,words::le_word(before,0)==c.count,
        after.subrange(0,4)==before.subrange(0,4),
        crate::fill::frame(before.subrange(4,before.len() as int),after.subrange(4,after.len() as int),
            c.width.unwrap() as int,c.key as int),
        validation::local_valid(before,layout,true),budget>0,
        forall|i:int|0<=i<c.count ==> #[trigger] validation::schema_valid(
            after.subrange(slot_start(c,i,i*c.width.unwrap()),slot_start(c,i,i*c.width.unwrap())+c.width.unwrap()),
            validation::child_layout(layout,i),(budget-1) as nat,true),
    ensures validation::schema_valid(after,layout,budget,true),header_frame(before,after,c),
{
    hide(validation::schema_valid);hide(validation::local_valid);
    hide(crate::search::keys_canonical);hide(crate::search::keys_ordered);hide(crate::search::lex_less);
    let width=c.width.unwrap();
    let ends=Seq::new(c.count as nat+1,|i:int|i*width);
    assert forall|p:int|0<=p<4 || key_byte(c,p) implies #[trigger] after[p]==before[p] by {
        if 0 <= p < 4 { assert(after.subrange(0,4)[p]==after[p]); assert(before.subrange(0,4)[p]==before[p]); }
        if key_byte(c,p) {
            rewrite_key_bounds(c,p);
            let i=choose|i:int|0<=i<c.count && #[trigger] key_at(c,i)<=p<key_at(c,i)+c.key;
            vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(i,p-4-i*c.stride,c.stride as int);
            vstd::arithmetic::div_mod::lemma_small_mod((p-4-i*c.stride) as nat,c.stride as nat);
            assert((p-4) % (width+c.key) < c.key);
            assert(after.subrange(4,after.len() as int)[p-4]==before.subrange(4,before.len() as int)[p-4]);
        }
    }
    assert forall|i:int|0<=i<ends.len() implies 0<=#[trigger] ends[i]<=c.total by {
        assert(0<=i*width<=c.total) by(nonlinear_arith)
            requires 0<=i<=c.count,c.total==c.count*width,width>0;
    }
    assert forall|i:int|0<=i<ends.len()-1 implies #[trigger] ends[i]<=ends[i+1] by {
        assert(i*width<=(i+1)*width) by(nonlinear_arith) requires width>0;
    }
    assert(prefix_ends(c,ends));
    assert forall|i:int|0<=i<c.count implies #[trigger] validation::schema_valid(
        after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(layout,i),(budget-1) as nat,true) by {
        assert(ends[i]==i*width);
        assert(ends[i+1]==(i+1)*width);
        assert(ends[i+1]-ends[i]==width) by(nonlinear_arith)
            requires ends[i]==i*width,ends[i+1]==(i+1)*width;
        assert(validation::schema_valid(
            after.subrange(slot_start(c,i,i*c.width.unwrap()),slot_start(c,i,i*c.width.unwrap())+c.width.unwrap()),
            validation::child_layout(layout,i),(budget-1) as nat,true));
        assert(after.subrange(slot_start(c,i,ends[i]),slot_start(c,i,ends[i])+ends[i+1]-ends[i])
            ==after.subrange(slot_start(c,i,i*width),slot_start(c,i,i*width)+width));

    }
    rewrite_selected_preserves(before,after,c,ends,layout,budget);
}

#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn rewrite_fixed_fill_witness()
    ensures true,
{
    let bytes=[2u8,0,0,0,1,0,3,1];
    let layout=witnesses::FIXED_MAP;
    let admitted=RewritePlan::check_fixed_map(&bytes,&layout,2,2);
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid,6);
        let before=bytes@;
        assert(before==seq![2u8,0,0,0,1,0,3,1]);
        assert(crate::layout::descriptor_valid(layout,MAX_DEPTH as nat));
        assert(crate::map_layout::value_width(layout)==Some(1));
        assert(words::le_word(before,0)==2);
        assert(crate::map_layout::map_shape(before,layout));
        assert(admitted.is_ok());
        assert(words::le_word(before,0)==2);
        let after=seq![2u8,0,0,0,1,1,3,0];
        let c=RewritePlan{count:2,payload:4,total:2,len:8,key:1,stride:2,width:Some(1),map:true};
        assert(crate::search::same_prefix(seq![1u8],seq![3u8],0));
        assert(crate::search::lex_less(seq![1u8],seq![3u8]));
        assert(crate::search::encoded_key(before,layout,0)==seq![1u8]);
        assert(crate::search::encoded_key(before,layout,1)==seq![3u8]);
        assert(crate::search::keys_canonical(before,layout,2));
        assert(crate::search::keys_ordered(before,layout,2));
        assert(crate::layout::descriptor_valid(layout,MAX_DEPTH as nat));
        assert(crate::map_layout::map_shape(before,layout));
        assert(validation::local_valid(before,layout,true));
        reveal_with_fuel(crate::validation::schema_valid,3);
        assert forall|i:int|0<=i<2 implies #[trigger] validation::schema_valid(
            after.subrange(slot_start(c,i,i),slot_start(c,i,i)+1),validation::child_layout(layout,i),(MAX_DEPTH-1) as nat,true) by {
            assert(i==0 || i==1);
        }
        let left=before.subrange(4,8); let right=after.subrange(4,8);
        assert forall|p:int|0<=p<4 && p%2<1 implies #[trigger] right[p]==left[p] by {
            assert(p==0 || p==2);
        }
        assert(crate::fill::frame(left,right,1,1));
        assert(after.subrange(0,4) =~= before.subrange(0,4));
        rewrite_fixed_fill_preserves(before,after,c,layout,MAX_DEPTH as nat);
    }
}

/// Uniform cumulative ends include the implicit list terminal.
spec fn uniform_ends(c: RewritePlan, len: int) -> Seq<int> {
    Seq::new(c.count as nat + 1, |i:int| i * len)
}

/// Every repeated slot is contained and disjoint, including empty values.
#[verifier::spinoff_prover]
proof fn uniform_slot_partition(c: RewritePlan, len: int, i: int, j: int)
    requires coordinates(c), c.width.is_none(), 0 <= len, c.total == c.count * len,
        0 <= i < c.count, 0 <= j < c.count,
    ensures c.payload <= c.payload + i*len <= c.payload + (i+1)*len <= c.len,
        i < j ==> c.payload + (i+1)*len <= c.payload + j*len,
        c.payload + i*len == slot_start(c,i,i*len),
{
    assert(0 <= i*len <= (i+1)*len <= c.total) by(nonlinear_arith)
        requires 0 <= i < c.count, len >= 0, c.total == c.count * len;
    if i < j {
        assert((i+1)*len <= j*len) by(nonlinear_arith) requires i < j, len >= 0;
    }
}

/// Complete uniform directories and retained count/keys form a complete stream.
#[verifier::spinoff_prover]
proof fn uniform_complete_frame(before: Seq<u8>, after: Seq<u8>, c: RewritePlan, len: int)
    requires coordinates(c), c.width.is_none(), 0 <= len, c.total == c.count * len,
        words::le_word(before,0) == c.count, header_frame(before,after,c),
        directory(after,c,uniform_ends(c,len)),
    ensures rewrite_result(before,after,c,uniform_ends(c,len)),
{
    let ends=uniform_ends(c,len);
    assert forall|i:int| 0 <= i < ends.len() implies 0 <= #[trigger] ends[i] <= c.total by {
        assert(0 <= i*len <= c.count*len) by(nonlinear_arith)
            requires 0 <= i <= c.count, len >= 0;
    }
    assert forall|i:int| 0 <= i < ends.len()-1 implies #[trigger] ends[i] <= ends[i+1] by {
        assert(i*len <= (i+1)*len) by(nonlinear_arith) requires len >= 0;
    }
}

/// Uniform streams use the existing ancestor patches and used-length-last result.
#[verifier::spinoff_prover]
proof fn uniform_ancestor_publication(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,plan:Plan,c:RewritePlan,len:int)
    requires replacement_checked(before,plan), subtree_plan(plan), replacement_result(before,encoded,after,plan),
        validation::schema_valid(before.subrange(12,plan.old_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        c.width.is_none(), 0 <= len, c.total == c.count*len,
        rewrite_result(before.subrange(plan.region.start as int,plan.region.end as int),
            encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),c,uniform_ends(c,len)),
        corresponds(c,plan.trail.selected),
        forall|i:int| 0 <= i < c.count ==> #[trigger] validation::schema_valid(
            encoded.subrange(plan.region.start+c.payload+i*len,plan.region.start+c.payload+(i+1)*len),
            validation::child_layout(plan.trail.selected,i),(MAX_DEPTH-plan.trail.depth-1) as nat,true),
    ensures validation::schema_valid(after.subrange(12,plan.new_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        words::le_word(after,8)==plan.new_used,
{
    let ends=uniform_ends(c,len);
    assert forall|i:int| 0 <= i < c.count implies #[trigger] validation::schema_valid(
        encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i]),
        validation::child_layout(plan.trail.selected,i),(MAX_DEPTH-plan.trail.depth-1) as nat,true) by {
        uniform_slot_partition(c,len,i,i);
        rewrite_slots_partition(c,ends,i);
        assert(slot_start(c,i,ends[i])+ends[i+1]-ends[i] == c.payload+(i+1)*len) by(nonlinear_arith)
            requires slot_start(c,i,ends[i]) == c.payload+i*len, ends[i]==i*len, ends[i+1]==(i+1)*len;
        assert(encoded.subrange(plan.region.start+slot_start(c,i,ends[i]),plan.region.start+slot_start(c,i,ends[i])+ends[i+1]-ends[i])
            == encoded.subrange(plan.region.start+c.payload+i*len,plan.region.start+c.payload+(i+1)*len));
    }
    rewrite_publish(before,encoded,after,plan,c,ends);
}

/// A failed or incomplete stream cannot authorize final-total publication.
#[verifier::spinoff_prover]
proof fn uniform_final_total(stream: RewriteStream, result: Result<()>)
    requires stream_checked(stream), stream_finished(stream,result),
    ensures result.is_ok() == (stream.pending.is_none() && stream.done==stream.plan.count && stream.end==stream.plan.total),
        result.is_ok() ==> rewrite_result(stream.original@,old(stream.bytes)@,stream.plan,stream.ends@),
{
}

#[cfg(verus_only)]
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn uniform_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal_with_fuel(crate::utf8::utf8_valid,4);
    let layout=witnesses::INDEXED_LIST;
    let old=seq![2u8,0,0,0,1,0,0,0,97,98];
    let new=seq![2u8,0,0,0,1,0,0,0,120,120];
    let c=RewritePlan {count:2,payload:8,total:2,len:10,key:0,stride:4,width:None,map:false};
    crate::utf8::ascii_valid(seq![97u8]); crate::utf8::ascii_valid(seq![98u8]); crate::utf8::ascii_valid(seq![120u8]);
    uniform_slot_partition(c,1,0,1);
    uniform_complete_frame(old,new,c,1);
    let before=witnesses::account(old);
    let after=witnesses::account(new);
    let plan=witnesses::replacement(before,layout,10);
    assert(before.subrange(12,22) =~= old);
    assert(after.subrange(12,22) =~= new);
    uniform_ancestor_publication(before,after,after,plan,c,1);
}

#[cfg(verus_only)]
fn uniform_completion_witness() ensures true,
{
    let mut bytes=[2u8,0,0,0,1,0,0,0,120,120];
    let c=RewritePlan {count:2,payload:8,total:2,len:10,key:0,stride:4,width:None,map:false};
    proof { broadcast use vstd::array::group_array_axioms; assert(bytes@==seq![2u8,0,0,0,1,0,0,0,120,120]); }
    let ghost before=bytes@;
    let ghost empty=seq![0int];
    let stream=RewriteStream {bytes:&mut bytes,plan:c,done:0,end:0,pending:None,issued:false,
        original:Ghost(before),ends:Ghost(empty)};
    proof {assert(stream_checked(stream));}
    let ghost initial=stream;
    let result=stream.finish();
    proof { uniform_final_total(initial,result); assert(result.is_err()); }
    let mut bytes=[2u8,0,0,0,1,0,0,0,120,120];
    proof { broadcast use vstd::array::group_array_axioms; assert(bytes@==seq![2u8,0,0,0,1,0,0,0,120,120]); }
    let ghost before=bytes@;
    let ghost ends=seq![0int,1,2];
    let stream=RewriteStream {bytes:&mut bytes,plan:c,done:2,end:2,pending:None,issued:false,
        original:Ghost(before),ends:Ghost(ends)};
    proof {assert(stream_checked(stream));}
    let ghost complete=stream;
    let result=stream.finish();
    proof { uniform_final_total(complete,result); assert(result.is_ok()); }
}

}
