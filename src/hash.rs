use num_bigint::BigUint;

pub trait Hashable {
    fn hash(&self) -> [u8; 32] {
        self.try_hash().unwrap()
    }
    fn try_hash(&self) -> Option<[u8; 32]> {
        None
    }
}

const COEFF_MASK:u32 = 0x007f_ffff;

pub fn bits_to_target(bits: u32) -> BigUint {
    let exp = (bits >> 24) as u8;
    let coeff = bits & COEFF_MASK;

    assert!(coeff < COEFF_MASK);

    BigUint::from(coeff) << (8 * (exp - 3))
}

pub fn target_to_bits(target:BigUint) -> u32 {
    let (exp,coeff) = match target.to_bytes_be().len() {
        0 => (0,0),
        len => {
            let mut size = len;
            let mut max_coeff = target.clone();
            while size > 3 {
                max_coeff >>= 8;
                size -= 1;
                if max_coeff <= BigUint::from(COEFF_MASK) {
                    break;
                }
            }
            
            // coeff must be < u32
            let coeff = max_coeff.to_u32_digits().first().copied().unwrap_or(COEFF_MASK);

            (size as u32 + 3, coeff)
        }
    };
    (exp<<24) | coeff
}
