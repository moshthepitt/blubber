//! Framed value loans for fixed and uniform collection fills.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Fallible encoder called once for each exact measured value destination.
/// A returned count is checked before the enclosing rewrite can publish.
pub trait ValueEncoder {
    /// Stable dimensions of one key/value entry.
    spec fn dimensions(&self) -> (u32,u32);
    /// Returns `(value_width, key_width)` without invoking input measurement.
    fn geometry(&self) -> (result:(u32,u32))
        ensures result == self.dimensions(),
        no_unwind;

    /// Encodes one value; failures and unwinds leave the owning store poisoned.
    fn encode(&self, slot: &mut [u8]) -> Result<usize>
        ensures final(slot).len() == old(slot).len();
}

/// Number of stored ends; list terminals remain implicit.
pub(crate) open spec fn uniform_stores(count:int,key:int)->int {
    if key==0 {count-1} else {count}
}

/// Complete directory prefix, with every key and remaining entry retained.
pub(crate) open spec fn uniform_directory(before:Seq<u8>,after:Seq<u8>,width:int,count:int,key:int,done:int)->bool {
    &&& frame(before,after,4,key)
    &&& forall|i:int| 0<=i<done && i<uniform_stores(count,key)
        ==> #[trigger] crate::words::le_word(after,i*(key+4)+key)==(i+1)*width
    &&& forall|p:int| 0<=p<before.len() && done*(key+4)<=p
        ==> #[trigger] after[p]==before[p]
}

/// A contained word has the same value in its enclosing sequence.
pub(crate) proof fn uniform_word_window(full:Seq<u8>,part:Seq<u8>,base:int,at:int)
    requires 0<=base,base+part.len()<=full.len(),part==full.subrange(base,base+part.len()),
        0<=at,at+4<=part.len(),
    ensures crate::words::le_word(full,base+at)==crate::words::le_word(part,at),
{
}

/// Paired, exact payload/directory loans preserve the completed prefix on every
/// exit. Directory entries advance before the next callback, including unwinds.
/// The caller admits the complete geometry before lending either slice.
#[doc(hidden)]
#[cfg_attr(target_os = "solana", inline(never))]
#[verifier::spinoff_prover]
#[verifier::rlimit(180)]
pub(crate) fn uniform_stream<E: ValueEncoder>(directory:&mut[u8],payload:&mut[u8],width:u32,count:u32,key:u32,encoder:&E)
    -> (result:(usize,Result<()>))
    requires 0<count,key+4<=u32::MAX,old(directory).len()<=u32::MAX,old(payload).len()<=u32::MAX,
        old(directory).len()==uniform_stores(count as int,key as int)*(key+4),
        old(payload).len()==count*width,
    ensures result.0<=count,result.1.is_ok()==(result.0==count),
        final(payload).len()==old(payload).len(),
        uniform_directory(old(directory)@,final(directory)@,width as int,count as int,key as int,result.0 as int),
{
    let geometry=encoder.geometry();
    if geometry.1!=key {return (0,Err(Error::new(ErrorKind::InvalidSchema,Error::NO_OFFSET)));}
    let key=geometry.1 as usize;
    let implicit=key==0;
    let stride=crate::words::add_bounded(key,4);
    let original_directory=directory.len();
    let total=payload.len();
    let ghost before=directory@;
    let ghost future=final(directory)@;
    let ghost payload_future=final(payload)@;
    let ghost stores=uniform_stores(count as int,key as int);
    let ghost mut prefix=Seq::<u8>::empty();
    let ghost mut payload_prefix=Seq::<u8>::empty();
    let ghost mut done=0int;
    let mut outcome=Ok(());
    let completed;
    {
    let mut directory_rest=&mut *directory;
    let mut payload_rest=&mut *payload;
    while !directory_rest.is_empty() && outcome.is_ok()
        invariant 0<count,implicit==(key==0),0<=done<=stores,stores==uniform_stores(count as int,key as int),0<=stores<=count,
            key+4==stride,stride<=u32::MAX,stride>0,key==geometry.1,
            original_directory==before.len()==stores*stride,
            total==count*width,total<=u32::MAX,
            directory_rest.len()==(stores-done)*stride,
            payload_rest.len()==(count-done-if outcome.is_ok(){0int}else{1int})*width,
            outcome.is_err() ==> done<stores,
            prefix.len()==done*stride,prefix.len()+directory_rest.len()==before.len(),
            payload_prefix.len()+payload_rest.len()==total,
            future==prefix+final(directory_rest)@,
            payload_future==payload_prefix+final(payload_rest)@,
            directory_rest@==before.subrange(prefix.len() as int,before.len() as int),
            frame(before.take(prefix.len() as int),prefix,4,key as int),
            forall|i:int|0<=i<done ==> #[trigger] crate::words::le_word(prefix,i*stride+key)==(i+1)*width,
        ensures outcome.is_ok() ==> directory_rest.len()==0,
        decreases directory_rest.len(),
    {
        proof {
            assert(done<stores) by(nonlinear_arith) requires directory_rest.len()>0,directory_rest.len()==(stores-done)*stride,stride>0;
            assert(width<=payload_rest.len()) by(nonlinear_arith)
                requires done<count,payload_rest.len()==(count-done)*width;
        }
        let ghost remaining=payload_rest.len();
        let (slot,tail)=payload_rest.split_at_mut(width as usize);
        let written=encoder.encode(slot);
        proof {
            payload_prefix=payload_prefix+slot@;
            assert(tail.len()==(count-done-1)*width) by(nonlinear_arith)
                requires remaining==(count-done)*width,tail.len()==remaining-width;
        }
        payload_rest=tail;
        match written {
            Err(error)=>{outcome=Err(error);break;},
            Ok(n)=>if n!=width as usize {outcome=Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));break;},
        }
        let next=crate::words::sub_bounded(total,payload_rest.len());
        proof {
            assert(next==(done+1)*width) by(nonlinear_arith)
                requires next==total-payload_rest.len(),total==count*width,payload_rest.len()==(count-done-1)*width;
            assert(stride<=directory_rest.len()) by(nonlinear_arith)
                requires done<stores,directory_rest.len()==(stores-done)*stride,stride>0;
        }
        let ghost directory_length=directory_rest.len();
        let (entry,rest)=directory_rest.split_at_mut(stride);
        let (_keys,word)=entry.split_at_mut(key);
        crate::words::write_contained_word(word,0,next as u32);
        proof {
            let previous=prefix;
            let updated=prefix+_keys@+word@;
            assert(updated.len()==(done+1)*stride) by(nonlinear_arith)
                requires prefix.len()==done*stride,_keys.len()+word.len()==stride,
                    updated.len()==prefix.len()+_keys.len()+word.len();
            assert(updated.subrange(0,prefix.len() as int)==prefix);
            assert(updated.subrange(prefix.len()+key,updated.len() as int)==word@);
            assert forall|i:int|0<=i<done+1 implies #[trigger] crate::words::le_word(updated,i*stride+key)==(i+1)*width by {
                if i<done {
                    assert(i*stride+key+4<=prefix.len()) by(nonlinear_arith)
                        requires i<done,i>=0,prefix.len()==done*stride,stride==key+4;
                    uniform_word_window(updated,prefix,0,i*stride+key);
                } else {
                    assert(i==done);
                    uniform_word_window(updated,word@,prefix.len()+key,0);
                }
            }
            assert forall|p:int|0<=p<updated.len() && p%(stride as int)<key
                implies #[trigger] updated[p]==before[p] by {
                if p>=prefix.len() {
                    vstd::arithmetic::div_mod::lemma_mod_multiples_basic(done,stride as int);
                    vstd::arithmetic::div_mod::lemma_mod_adds(prefix.len() as int,p-prefix.len(),stride as int);
                    vstd::arithmetic::div_mod::lemma_small_mod((p-prefix.len()) as nat,stride as nat);
                    assert(p-prefix.len()<key);
                }
            }
            assert(rest.len()==(stores-(done+1))*stride) by(nonlinear_arith)
                requires directory_length==(stores-done)*stride,rest.len()==directory_length-stride;
            prefix=updated;
            done=done+1;
        }
        directory_rest=rest;
    }
    if outcome.is_ok() {
        proof {assert(done==stores) by(nonlinear_arith)
            requires directory_rest.len()==0,directory_rest.len()==(stores-done)*stride,stride>0;}
        if implicit {
            proof {assert(payload_rest.len()==width) by(nonlinear_arith)
                requires payload_rest.len()==(count-done)*width,done==stores,stores==count-1;}
            let written=encoder.encode(payload_rest);
            match written {
                Err(error)=>outcome=Err(error),
                Ok(n)=>if n!=width as usize {outcome=Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));},
            }
        }
    }
    completed=if outcome.is_ok() {count as usize} else {
        let consumed=crate::words::sub_bounded(original_directory,directory_rest.len());
        proof {
            assert(consumed==stride*done) by(nonlinear_arith)
                requires consumed==original_directory-directory_rest.len(),original_directory==stores*stride,
                    directory_rest.len()==(stores-done)*stride;
            vstd::arithmetic::div_mod::lemma_div_multiples_vanish(done,stride as int);
        }
        consumed/stride
    };
    proof {
        assert(outcome.is_err() ==> completed==done);
        assert(completed>=done);
        assert(completed<=count);
        assert(outcome.is_ok()==(completed==count));
        assert forall|p:int|0<=p<before.len() && p%(stride as int)<key implies #[trigger] future[p]==before[p] by {}
        assert forall|i:int|0<=i<completed && i<stores implies #[trigger] crate::words::le_word(future,i*stride+key)==(i+1)*width by {
            assert(i<done);
            assert(i*stride+key+4<=prefix.len()) by(nonlinear_arith)
                requires i<done,i>=0,prefix.len()==done*stride,stride==key+4;
            assert(future.subrange(0,prefix.len() as int)==prefix);
            uniform_word_window(future,prefix,0,i*stride+key);
        }
        assert forall|p:int|0<=p<before.len() && completed*stride<=p implies #[trigger] future[p]==before[p] by {
            assert(p>=prefix.len()) by(nonlinear_arith)
                requires p>=completed*stride,completed>=done,prefix.len()==done*stride,stride>0;
        }
    }
    }
    (completed,outcome)
}

/// Bytes outside the value intervals remain unchanged, including every key.
pub open spec fn frame(before: Seq<u8>, after: Seq<u8>, width: int, key: int) -> bool {
    &&& after.len() == before.len()
    &&& forall|p: int| 0 <= p < before.len() && p % (width + key) < key
        ==> #[trigger] after[p] == before[p]
}

/// Writes fixed values through exact loans, retaining every intervening key.
/// The caller admits entry geometry before encoding. Each reported length is checked.
#[doc(hidden)]
// The pinned verifier models remainder arithmetic directly.
#[allow(clippy::manual_is_multiple_of)]
#[cfg_attr(target_os = "solana", inline(never))]
pub fn values<E: ValueEncoder>(bytes: &mut [u8], encoder: &E)
    -> (result: Result<()>)
    requires 0 < encoder.dimensions().0, encoder.dimensions().0 + encoder.dimensions().1 <= u32::MAX,
        old(bytes).len() <= u32::MAX,
        (old(bytes).len() as int) % (encoder.dimensions().0 + encoder.dimensions().1) == 0,
    ensures frame(old(bytes)@, final(bytes)@, encoder.dimensions().0 as int, encoder.dimensions().1 as int),

{
    let (width,key)=encoder.geometry();
    if width==0 { return Err(Error::new(ErrorKind::InvalidSchema,Error::NO_OFFSET)); }
    let width=width as usize;
    let key=key as usize;
    let stride=crate::words::add_bounded(width,key);
    if bytes.len()%stride!=0 { return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET)); }
    let ghost before=bytes@;
    let ghost future=final(bytes)@;
    let ghost mut prefix=Seq::<u8>::empty();
    let mut outcome=Ok(());
    {
    let mut rest=&mut *bytes;
    while rest.len()>=stride
        invariant width == encoder.dimensions().0, key == encoder.dimensions().1, 0<width,stride==width+key,stride<=u32::MAX,before==old(bytes)@,
            rest.len()<=u32::MAX,rest.len()%stride==0,
            prefix.len()+rest.len()==before.len(),
            (prefix.len() as int)%(stride as int)==0,
            future==prefix+final(rest)@,
            frame(before.take(prefix.len() as int),prefix,width as int,key as int),
            rest@==before.subrange(prefix.len() as int,before.len() as int),
        decreases rest.len(),
    {
        proof {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(rest.len() as int,stride as int);
            assert(stride<=rest.len()) by(nonlinear_arith)
                requires rest.len()>0,stride>0,rest.len() as int==(rest.len() as int/stride as int)*stride;
        }
        let ghost length=rest.len();
        let (head,tail)=rest.split_at_mut(stride);
        let (_keys,slot)=head.split_at_mut(key);
        let written=encoder.encode(slot);
        proof {
            let next=prefix+_keys@+slot@;
            assert forall|p:int|0<=p<next.len() && p%(stride as int)<key implies #[trigger] next[p]==before[p] by {
                if p>=prefix.len() {
                    vstd::arithmetic::div_mod::lemma_mod_adds(prefix.len() as int,p-prefix.len(),stride as int);
                    vstd::arithmetic::div_mod::lemma_small_mod((p-prefix.len()) as nat,stride as nat);
                    assert(p-prefix.len()<key);
                }
            }
            assert(frame(before.take(next.len() as int),next,width as int,key as int));
            vstd::arithmetic::div_mod::lemma_mod_adds(prefix.len() as int,stride as int,stride as int);
            vstd::arithmetic::div_mod::lemma_mod_sub_multiples_vanish(length as int,stride as int);
            prefix=next;
        }
        rest=tail;
        match written {Err(e)=>{outcome=Err(e);break;},Ok(n)=>if n!=width{outcome=Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));break;}}
    }
    proof {
        assert forall|p:int|0<=p<before.len() && p%(stride as int)<key implies #[trigger] future[p]==before[p] by {}
    }
    }
    outcome
}

/// Every fixed value is contained in its entry and misses every key interval.
pub proof fn fill_stride_containment(count: int, width: int, key: int, i: int, j: int)
    requires 0 < width, 0 <= key, 0 <= i < count, 0 <= j < count,
        count * (width + key) <= u32::MAX,
    ensures 0 <= i * (width + key) + key < (i+1) * (width+key) <= count * (width+key),
        j * (width+key) + key <= i * (width+key) + key || (i+1) * (width+key) <= j * (width+key),
{
    assert(0 <= i * (width+key) + key < (i+1) * (width+key) <= count * (width+key)) by(nonlinear_arith)
        requires 0 < width, 0 <= key, 0 <= i < count;
    if j <= i {
        assert(j * (width+key) + key <= i * (width+key) + key) by(nonlinear_arith)
            requires j <= i, width > 0, key >= 0;
    } else {
        assert((i+1) * (width+key) <= j * (width+key)) by(nonlinear_arith)
            requires i < j, width > 0, key >= 0;
    }
}

/// A contained collection write retains the account header and physical spare.
pub proof fn fill_account_frame(before: Seq<u8>, after: Seq<u8>, start: int, end: int, used: int)
    requires before.len() == after.len(), 12 <= start <= end <= used <= before.len(),
        forall|p: int| 0 <= p < before.len() && !(start <= p < end)
            ==> #[trigger] after[p] == before[p],
    ensures after.subrange(0,12) == before.subrange(0,12),
        after.subrange(used,before.len() as int) == before.subrange(used,before.len() as int),
{
    assert(after.subrange(0,12) =~= before.subrange(0,12));
    assert(after.subrange(used,before.len() as int) =~= before.subrange(used,before.len() as int));
}

#[cfg(verus_only)]
fn fill_witness()
    ensures true,
{
    proof {
        fill_stride_containment(2,3,1,1,0);
        let before = seq![0u8,0,0,0,0,0,0,0,20,0,0,0,2,0,0,0,1,7,3,9,165];
        let after = before.update(17,42u8).update(19,42u8);
        fill_account_frame(before,after,16,20,20);
    }
}

}
