//! KAGEMUSHA V1 attested-app suite prototype.

#[cfg(test)]
mod proto {
    use norito::codec::{Decode, Encode};

    /// Body.
    #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
    #[norito(decode_from_slice)]
    #[norito_schema(name = "proto::Body")]
    pub struct Body {
        /// v
        pub version: u16,
        /// id
        pub id: [u8; 32],
        /// s
        pub label: String,
        /// n
        pub n: u64,
    }

    /// Signed.
    #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(name = "proto::Signed")]
    pub struct Signed {
        /// body
        #[norito(flatten)]
        pub body: Body,
        /// sig
        pub signature: [u8; 64],
    }

    /// Item
    #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(name = "proto::Item")]
    pub struct Item {
        /// d
        pub digest: [u8; 32],
    }

    /// Holder
    #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(name = "proto::Holder")]
    pub struct Holder {
        /// s
        pub signed: Signed,
        /// o
        pub opt: Option<Signed>,
        /// items
        pub items: Vec<Item>,
        /// flag
        pub flag: bool,
    }

    /// Enum
    #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(name = "proto::Ev")]
    pub enum Ev {
        /// a
        A {
            /// x
            x: Signed,
            /// s
            sig: [u8; 64],
        },
        /// b
        B {
            /// y
            y: u64,
        },
    }

    #[test]
    fn layout() {
        let body = Body {
            version: 1,
            id: [7; 32],
            label: "abc".into(),
            n: 5,
        };
        let signed = Signed {
            body: body.clone(),
            signature: [9; 64],
        };
        let bf = norito::encode_canonical(&body).unwrap();
        let sf = norito::encode_canonical(&signed).unwrap();
        println!("body frame {} {}", bf.len(), hex::encode(&bf));
        println!("signed frame {} {}", sf.len(), hex::encode(&sf));
        assert_eq!(&sf[40..40 + bf.len() - 40], &bf[40..]);
        let back: Signed = norito::decode_canonical(&sf).unwrap();
        assert_eq!(back, signed);
        let holder = Holder {
            signed: signed.clone(),
            opt: Some(signed.clone()),
            items: vec![Item { digest: [3; 32] }, Item { digest: [4; 32] }],
            flag: true,
        };
        let hf = norito::encode_canonical(&holder).unwrap();
        println!("holder frame {} {}", hf.len(), hex::encode(&hf));
        let hb: Holder = norito::decode_canonical(&hf).unwrap();
        assert_eq!(hb, holder);
        let holder2 = Holder {
            opt: None,
            items: vec![],
            ..holder
        };
        let hf2 = norito::encode_canonical(&holder2).unwrap();
        println!("holder2 frame {} {}", hf2.len(), hex::encode(&hf2));
        let hb2: Holder = norito::decode_canonical(&hf2).unwrap();
        assert_eq!(hb2, holder2);
        let ev = Ev::A {
            x: signed.clone(),
            sig: [1; 64],
        };
        let ef = norito::encode_canonical(&ev).unwrap();
        println!("ev frame {} {}", ef.len(), hex::encode(&ef));
        let eb: Ev = norito::decode_canonical(&ef).unwrap();
        assert_eq!(eb, ev);
        let vv: Vec<[u8; 32]> = vec![[5; 32]];
        let vf = norito::to_bytes(&vv);
        println!("vec arr {:?}", vf.map(|b| hex::encode(b)));
    }
}
