use std::borrow::Borrow;

use alloc::vec::Vec;
use bytes::{Bytes};

use dns_message_parser::{Dns, Flags, Opcode, RCode, rr::RR};
use dns_message_parser::question::{QClass, QType, Question};


pub fn make_tcp_dns_request(domain_name: &str) -> Vec<u8> {

    // TODO: make this random
    let id = 0x0001;

    let flags = Flags {
        qr: false,
        opcode: Opcode::Query,
        aa: true,
        tc: false,
        rd: true,
        ra: true,
        ad: false,
        cd: false,
        rcode: RCode::NoError,
    };

    let question = {
        let domain_name = domain_name.parse().unwrap();
        let q_class = QClass::IN;
        let q_type = QType::A;

        Question {
            domain_name,
            q_class,
            q_type,
        }
    };

    let questions = vec![question];
    let dns = Dns {
        id,
        flags,
        questions,
        answers: Vec::new(),
        authorities: Vec::new(),
        additionals: Vec::new(),
    };
    let dns_bytes = dns.encode().unwrap();
    let dns_bytes: &[u8] = dns_bytes.borrow();

    let q_len: u16 = dns_bytes.len().try_into().unwrap();

    let tcp_bytes = [&u16::to_be_bytes(q_len), dns_bytes].concat();

    tcp_bytes
}

pub fn parse_tcp_dns_response(buf: &[u8]) -> [u8; 4] {
    let bytes = Bytes::copy_from_slice(buf);
    let dns = Dns::decode(bytes).expect("Invalid DNS response");
    match dns.answers.get(0) {
        Some(RR::A(ans)) => ans.ipv4_addr.octets(),
        _ => panic!("Invalid DNS response"),
    }
}
