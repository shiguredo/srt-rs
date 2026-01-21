use crate::error::Error;

pub trait ByteSliceExt {
    fn read_u8(&mut self) -> Result<u8, Error>;
    fn read_u16(&mut self) -> Result<u16, Error>;
    fn read_u32(&mut self) -> Result<u32, Error>;
    fn read_u64(&mut self) -> Result<u64, Error>;
    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, Error>;
    fn read_utf8(&mut self, len: usize) -> Result<String, Error>;
}

impl ByteSliceExt for &[u8] {
    #[track_caller]
    fn read_u8(&mut self) -> Result<u8, Error> {
        Error::check_buffer_size(1, self)?;
        let v = self[0];
        *self = &self[1..];
        Ok(v)
    }

    #[track_caller]
    fn read_u16(&mut self) -> Result<u16, Error> {
        Error::check_buffer_size(2, self)?;
        let bytes = [self[0], self[1]];
        *self = &self[2..];
        Ok(u16::from_be_bytes(bytes))
    }

    #[track_caller]
    fn read_u32(&mut self) -> Result<u32, Error> {
        Error::check_buffer_size(4, self)?;
        let bytes = [self[0], self[1], self[2], self[3]];
        *self = &self[4..];
        Ok(u32::from_be_bytes(bytes))
    }

    #[track_caller]
    fn read_u64(&mut self) -> Result<u64, Error> {
        Error::check_buffer_size(8, self)?;
        let bytes = [
            self[0], self[1], self[2], self[3], self[4], self[5], self[6], self[7],
        ];
        *self = &self[8..];
        Ok(u64::from_be_bytes(bytes))
    }

    #[track_caller]
    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, Error> {
        Error::check_buffer_size(len, self)?;
        let buf = self[..len].to_vec();
        *self = &self[len..];
        Ok(buf)
    }

    #[track_caller]
    fn read_utf8(&mut self, len: usize) -> Result<String, Error> {
        let buf = self.read_bytes(len)?;
        String::from_utf8(buf).map_err(|e| Error::invalid_data(format!("invalid UTF-8 bytes: {e}")))
    }
}

pub trait VecExt {
    fn write_u8(&mut self, v: u8);
    fn write_u16(&mut self, v: u16);
    fn write_u32(&mut self, v: u32);
    fn write_u64(&mut self, v: u64);
    fn write_bytes(&mut self, v: &[u8]);
}

impl VecExt for Vec<u8> {
    fn write_u8(&mut self, v: u8) {
        self.push(v);
    }

    fn write_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_be_bytes());
    }

    fn write_u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_be_bytes());
    }

    fn write_u64(&mut self, v: u64) {
        self.extend_from_slice(&v.to_be_bytes());
    }

    fn write_bytes(&mut self, v: &[u8]) {
        self.extend_from_slice(v);
    }
}
