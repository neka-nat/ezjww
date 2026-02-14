use std::io::Cursor;

use encoding_rs::SHIFT_JIS;

use crate::error::JwwError;

pub struct Reader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub fn bytes_read(&self) -> usize {
        self.cursor.position() as usize
    }

    pub fn skip(&mut self, len: usize) -> Result<(), JwwError> {
        let pos = self.bytes_read();
        let new_pos = pos
            .checked_add(len)
            .ok_or(JwwError::UnexpectedEof("offset"))?;
        if new_pos > self.cursor.get_ref().len() {
            return Err(JwwError::UnexpectedEof("bytes"));
        }
        self.cursor.set_position(new_pos as u64);
        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8, JwwError> {
        Ok(self.read_exact::<1>()?[0])
    }

    pub fn read_u16(&mut self) -> Result<u16, JwwError> {
        Ok(u16::from_le_bytes(self.read_exact::<2>()?))
    }

    pub fn read_u32(&mut self) -> Result<u32, JwwError> {
        Ok(u32::from_le_bytes(self.read_exact::<4>()?))
    }

    pub fn read_f64(&mut self) -> Result<f64, JwwError> {
        Ok(f64::from_le_bytes(self.read_exact::<8>()?))
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, JwwError> {
        let mut buf = vec![0_u8; len];
        self.read_exact_into(&mut buf)?;
        Ok(buf)
    }

    pub fn read_cstring(&mut self) -> Result<String, JwwError> {
        let len_byte = self.read_u8()?;
        let len = if len_byte < 0xFF {
            len_byte as usize
        } else {
            let word_len = self.read_u16()?;
            if word_len < 0xFFFF {
                word_len as usize
            } else {
                self.read_u32()? as usize
            }
        };

        if len == 0 {
            return Ok(String::new());
        }

        let bytes = self.read_bytes(len)?;
        let (decoded, _, _) = SHIFT_JIS.decode(&bytes);
        Ok(decoded.trim_end_matches('\0').to_string())
    }

    fn read_exact<const N: usize>(&mut self) -> Result<[u8; N], JwwError> {
        let mut buf = [0_u8; N];
        self.read_exact_into(&mut buf)?;
        Ok(buf)
    }

    fn read_exact_into(&mut self, buf: &mut [u8]) -> Result<(), JwwError> {
        let pos = self.bytes_read();
        let end = pos
            .checked_add(buf.len())
            .ok_or(JwwError::UnexpectedEof("offset"))?;
        let src = self.cursor.get_ref();
        if end > src.len() {
            return Err(JwwError::UnexpectedEof("bytes"));
        }
        buf.copy_from_slice(&src[pos..end]);
        self.cursor.set_position(end as u64);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn read_numeric_values() {
        let data = [
            0x01, // u8
            0x02, 0x00, // u16
            0x58, 0x02, 0x00, 0x00, // u32 (600)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, // f64 (1.0)
        ];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_u8().unwrap(), 1);
        assert_eq!(reader.read_u16().unwrap(), 2);
        assert_eq!(reader.read_u32().unwrap(), 600);
        assert_eq!(reader.read_f64().unwrap(), 1.0);
    }

    #[test]
    fn read_cstring_short() {
        let data = [4, b't', b'e', b's', b't'];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), "test");
    }

    #[test]
    fn read_cstring_empty() {
        let data = [0];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.read_cstring().unwrap(), "");
    }
}
