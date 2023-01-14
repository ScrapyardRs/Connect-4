#![feature(macro_metavar_expr)]

use drax::transport::packet::string::LimitedString;

pub extern crate drax;

#[macro_export]
macro_rules! encode {
    ($write:expr, $ty:ty, $packet:expr) => {
        let _ = core::drax::prelude::DraxWriteExt::encode_component::<(), $ty>(
            &mut $write,
            &mut (),
            &$packet,
        )
        .await;
    };
}

pub mod logger;

pub type Username = LimitedString<16>;

pub mod packets {
    use drax::transport::packet::primitive::VarInt;

    drax::components! {
        enum ServerboundLoginPacket<key: VarInt> {
            KeepAlive {},
            RequestUsername {
                username: super::Username,
                transaction_id: VarInt
            },
            AcquireUsername {}
        },

        enum ClientboundLoginPacket<key: VarInt> {
            KeepAlive {},
            UsernameResult {
                success: bool,
                transaction_id: i32
            }
        },

        enum ServerboundLobbyPacket<key: VarInt> {
            KeepAlive {},
            RequestGame {},
            AcquireGame {}
        },

        enum ClientboundLobbyPacket<key: VarInt> {
            KeepAlive {},
            GameFound {}
        },

        enum ServerboundGamePacket<key: VarInt> {
            KeepAlive {},
            PlacePiece {
                column: u8,
                transaction_id: i32
            },
            AcquireLobby {}
        },

        enum ClientboundGamePacket<key: VarInt> {
            KeepAlive {},
            OpponentJoin {
                username: super::Username
            },
            PlacePieceAck {
                transaction_id: i32
            },
            OpponentPlacedPiece {
                column: u8
            },
            EarlyExit {},
            PlayerWin {
                me: bool
            }
        }
    }
}
