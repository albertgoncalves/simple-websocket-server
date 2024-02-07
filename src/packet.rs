use std::io::{ErrorKind, Read};

#[derive(Clone, Copy)]
struct Metadata {
    fin: bool,
    opcode: Opcode,
    masked: bool,
    len: u64,
}

#[derive(Debug, PartialEq)]
pub enum Packet {
    Text(String),
    Close(u16, Option<String>),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Opcode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl From<Opcode> for u8 {
    fn from(opcode: Opcode) -> Self {
        match opcode {
            Opcode::Continuation => 0,
            Opcode::Text => 1,
            Opcode::Binary => 2,
            Opcode::Close => 8,
            Opcode::Ping => 9,
            Opcode::Pong => 10,
        }
    }
}

impl From<u8> for Opcode {
    fn from(opcode: u8) -> Self {
        match opcode {
            0 => Self::Continuation,
            1 => Self::Text,
            2 => Self::Binary,
            8 => Self::Close,
            9 => Self::Ping,
            10 => Self::Pong,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Error {
    IO(ErrorKind),
    Int,
    Slice,
    ReservedBits,
    Length,
    Opcode,
    Utf8,
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IO(error.kind())
    }
}

impl From<std::array::TryFromSliceError> for Error {
    fn from(_: std::array::TryFromSliceError) -> Self {
        Self::Slice
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(_: std::num::TryFromIntError) -> Self {
        Self::Int
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(_: std::string::FromUtf8Error) -> Self {
        Self::Utf8
    }
}

pub fn serialize(opcode: Opcode, mask: Option<[u8; 4]>, payload: &[u8]) -> Vec<u8> {
    let fin = true;
    let len = payload.len();

    let mut buffer: Vec<u8> = Vec::with_capacity(14 + len);
    buffer.push((u8::from(fin) << 7) | u8::from(opcode));

    if let Some(mask) = mask {
        if len < 126 {
            buffer.push((1u8 << 7) | len as u8);
        } else {
            todo!()
        }
        buffer.extend_from_slice(&mask[..]);
        let mut payload: Vec<u8> = payload.to_owned();
        for i in 0..len {
            payload[i] ^= mask[i % 4];
        }
        buffer.extend_from_slice(&payload);
    } else {
        buffer.push(len as u8);
        buffer.extend_from_slice(payload);
    }

    buffer
}

fn read_metadata<R: Read>(mut reader: R) -> Result<Metadata, Error> {
    let (fin, opcode, masked, mut len) = {
        let mut buffer = [0; 2];
        reader.read_exact(&mut buffer)?;

        let fin = (buffer[0] & 0b1000_0000) != 0;

        if (buffer[0] & 0b0111_0000) != 0 {
            return Err(Error::ReservedBits);
        }

        let opcode = (buffer[0] & 0b000_1111).into();
        let masked = (buffer[1] & 0b1000_0000) != 0;
        let len: u64 = (buffer[1] & 0b0111_1111).into();

        (fin, opcode, masked, len)
    };

    if len == 126 {
        let mut buffer = [0; 2];
        reader.read_exact(&mut buffer)?;
        len = u16::from_be_bytes(buffer).into();
    } else if len == 127 {
        let mut buffer = [0; 8];
        reader.read_exact(&mut buffer)?;
        len = u64::from_be_bytes(buffer);
        if 0x7FFF_FFFF_FFFF_FFFF < len {
            return Err(Error::Length);
        }
    }

    Ok(Metadata {
        fin,
        opcode,
        masked,
        len,
    })
}

fn read_payload<R: Read>(metadata: Metadata, mut reader: R) -> Result<Vec<u8>, Error> {
    let len = metadata.len.try_into()?;
    Ok(if metadata.masked {
        let mask = {
            let mut buffer = [0; 4];
            reader.read_exact(&mut buffer)?;
            buffer
        };

        let mut buffer = vec![0; len];
        reader.read_exact(&mut buffer)?;
        for i in 0..len {
            buffer[i] ^= mask[i % 4];
        }
        buffer
    } else {
        let mut buffer = vec![0; len];
        reader.read_exact(&mut buffer)?;
        buffer
    })
}

pub fn read<R: Read>(mut reader: R) -> Result<Packet, Error> {
    let mut metadata = read_metadata(&mut reader)?;
    let mut payload = read_payload(metadata, &mut reader)?;
    let opcode = metadata.opcode;

    if !metadata.fin {
        loop {
            metadata = read_metadata(&mut reader)?;
            if metadata.opcode != Opcode::Continuation {
                return Err(Error::Opcode);
            }
            payload.extend_from_slice(&read_payload(metadata, &mut reader)?);
            if metadata.fin {
                break;
            }
        }
    }

    match opcode {
        Opcode::Text => Ok(Packet::Text(String::from_utf8(payload)?)),
        Opcode::Binary => todo!(),
        Opcode::Close => {
            let (status_code, payload) = payload[..metadata.len.try_into()?].split_at(2);
            Ok(Packet::Close(
                u16::from_be_bytes(status_code.try_into()?),
                String::from_utf8(payload.into())
                    .ok()
                    .filter(|string| !string.is_empty()),
            ))
        }
        Opcode::Ping => todo!(),
        Opcode::Pong => todo!(),
        Opcode::Continuation => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use crate::packet::{serialize, Opcode};

    #[test]
    fn read_hello() {
        assert_eq!(
            super::read(&[129, 6, 72, 101, 108, 108, 111, 33][..]),
            Ok(super::Packet::Text("Hello!".to_owned())),
        );
    }

    #[test]
    fn read_hello_masked() {
        assert_eq!(
            super::read(&[129, 134, 132, 29, 59, 30, 204, 120, 87, 114, 235, 60][..]),
            Ok(super::Packet::Text("Hello!".to_owned())),
        );
    }

    #[test]
    fn read_hello_fragmented() {
        assert_eq!(
            super::read(&[1, 3, 72, 101, 108, 128, 3, 108, 111, 33][..]),
            Ok(super::Packet::Text("Hello!".to_owned())),
        );
    }

    #[test]
    fn read_close_1001() {
        assert_eq!(
            super::read(&[136, 130, 247, 207, 169, 128, 244, 38][..]),
            Ok(super::Packet::Close(1001, None)),
        );
    }

    #[test]
    fn serialize_hello() {
        assert_eq!(
            serialize(Opcode::Text, None, b"Hello!"),
            [129, 6, 72, 101, 108, 108, 111, 33],
        );
    }

    #[test]
    fn serialize_hello_masked() {
        assert_eq!(
            serialize(Opcode::Text, Some([132, 29, 59, 30]), b"Hello!"),
            [129, 134, 132, 29, 59, 30, 204, 120, 87, 114, 235, 60],
        );
    }
}
