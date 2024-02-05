use std::io::Read;

#[derive(Clone, Copy)]
struct Metadata {
    fin: bool,
    opcode: u8,
    masked: bool,
    len: u64,
}

#[derive(Debug, PartialEq)]
pub enum Packet {
    Text(String),
    Close(u16, Option<String>),
}

fn read_metadata<R: Read>(mut reader: R) -> std::io::Result<Metadata> {
    let (fin, opcode, masked, mut len) = {
        let mut buffer = [0; 2];
        reader.read_exact(&mut buffer)?;

        let fin = (buffer[0] & (1 << 7)) != 0;

        assert_eq!((buffer[0] & (1 << 6)), 0);
        assert_eq!((buffer[0] & (1 << 5)), 0);
        assert_eq!((buffer[0] & (1 << 4)), 0);

        let opcode = buffer[0] & 0xF;
        let masked = (buffer[1] & (1 << 7)) != 0;
        let len: u64 = (buffer[1] & 0x7F).into();

        (fin, opcode, masked, len)
    };

    if len == 126 {
        let mut buffer = [0; 2];
        reader.read_exact(&mut buffer)?;
        len = u16::from_be_bytes(buffer).into()
    } else if len == 127 {
        let mut buffer = [0; 8];
        reader.read_exact(&mut buffer)?;
        len = u64::from_be_bytes(buffer);
        assert!(len <= 0x7FFFFFFFFFFFFFFF);
    }

    Ok(Metadata {
        fin,
        opcode,
        masked,
        len,
    })
}

fn read_payload<R: Read>(metadata: Metadata, mut reader: R) -> std::io::Result<Vec<u8>> {
    Ok(if metadata.masked {
        let mask = {
            let mut buffer = [0; 4];
            reader.read_exact(&mut buffer)?;
            buffer
        };

        let mut buffer = vec![0; metadata.len.try_into().unwrap()];
        reader.read_exact(&mut buffer)?;
        for i in 0..metadata.len.try_into().unwrap() {
            buffer[i] ^= mask[i % 4];
        }
        buffer
    } else {
        let mut buffer = vec![0; metadata.len.try_into().unwrap()];
        reader.read_exact(&mut buffer)?;
        buffer
    })
}

pub fn read_packet<R: Read>(mut reader: R) -> std::io::Result<Packet> {
    let mut metadata = read_metadata(&mut reader)?;
    let mut payload = read_payload(metadata, &mut reader)?;
    let opcode = metadata.opcode;

    if !metadata.fin {
        loop {
            metadata = read_metadata(&mut reader)?;
            assert_eq!(metadata.opcode, 0);
            payload.extend_from_slice(&read_payload(metadata, &mut reader)?);
            if metadata.fin {
                break;
            }
        }
    }

    match opcode {
        0 => unreachable!(),
        1 => Ok(Packet::Text(
            String::from_utf8(payload).map_err(|_| std::io::ErrorKind::InvalidData)?,
        )),
        2 => todo!(),
        8 => Ok(Packet::Close(
            u16::from_be_bytes(payload[..2].try_into().unwrap()),
            String::from_utf8(payload[2..metadata.len.try_into().unwrap()].to_vec())
                .ok()
                .filter(|string| !string.is_empty()),
        )),
        9 => todo!(),
        10 => todo!(),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn packet_hello() {
        assert_eq!(
            super::read_packet(&[129, 6, 72, 101, 108, 108, 111, 33][..]).unwrap(),
            super::Packet::Text("Hello!".to_owned()),
        )
    }

    #[test]
    fn packet_hello_masked() {
        assert_eq!(
            super::read_packet(&[129, 134, 132, 29, 59, 30, 204, 120, 87, 114, 235, 60][..])
                .unwrap(),
            super::Packet::Text("Hello!".to_owned()),
        )
    }

    #[test]
    fn packet_hello_fragmented() {
        assert_eq!(
            super::read_packet(&[1, 3, 72, 101, 108, 128, 3, 108, 111, 33][..]).unwrap(),
            super::Packet::Text("Hello!".to_owned()),
        )
    }

    #[test]
    fn packet_close_1001() {
        assert_eq!(
            super::read_packet(&[136, 130, 247, 207, 169, 128, 244, 38][..]).unwrap(),
            super::Packet::Close(1001, None),
        )
    }
}
