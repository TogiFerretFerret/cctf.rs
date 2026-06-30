use super::{EmailError, EmailService};
use async_trait::async_trait;
use base64::{Engine as _, prelude::BASE64_STANDARD};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf};
