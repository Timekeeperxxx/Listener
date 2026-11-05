use crate::{
    auth::Credentials,
    error::{AppError, Result},
    models::{Library, Track},
    qq::QqMusic,
};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes128Gcm, Nonce,
};
use base64::Engine;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};

// zzc/ag-1 wire format: https://github.com/jixunmoe/qmweb-sign (MIT).
const SCRAMBLE: [u8; 20] = [
    89, 39, 179, 150, 218, 82, 58, 252, 177, 52, 186, 123, 120, 64, 242, 133, 143, 161, 121, 179,
];
pub(crate) fn web_sign(body: &[u8]) -> String {
    let digest = Sha1::digest(body);
    let hex = format!("{:X}", digest);
    let pick = |indexes: &[usize]| {
        indexes
            .iter()
            .map(|i| hex.as_bytes()[*i] as char)
            .collect::<String>()
    };
    let middle = base64::engine::general_purpose::STANDARD
        .encode(
            digest
                .iter()
                .zip(SCRAMBLE)
                .map(|(a, b)| a ^ b)
                .collect::<Vec<_>>(),
        )
        .replace(['/', '+', '='], "");
    format!(
        "zzc{}{}{}",
        pick(&[23, 14, 6, 36, 16, 7, 19]),
        middle,
        pick(&[16, 1, 32, 12, 19, 27, 8, 5])
    )
    .to_lowercase()
}

impl QqMusic {
    pub async fn renew(&self, credentials: &Credentials) -> Result<Credentials> {
        let data = self
            .rpc(
                Some(credentials),
                credentials.comm(),
                "music.login.LoginServer",
                "Login",
                credentials.refresh_param()?,
            )
            .await?;
        let renewed = Credentials::from_login(data, credentials.login_type)?;
        if renewed.uin != credentials.uin {
            return Err(AppError::new(
                "SESSION_CHANGED",
                "续期返回的账户不一致，请重新扫码。",
            ));
        }
        Ok(renewed)
    }
    async fn write_playlist(
        &self,
        auth: &Credentials,
        module: &str,
        method: &str,
        param: Value,
    ) -> Result<Value> {
        let mut comm = auth.comm();
        for (key,value) in json!({"ct":24,"cv":4747474,"platform":"yqq.json","g_tk":auth.csrf_token().parse::<u64>().expect("numeric token"),"g_tk_new_20200303":auth.csrf_token().parse::<u64>().expect("numeric token"),"inCharset":"utf-8","outCharset":"utf-8","notice":0,"needNewCode":1}).as_object().expect("object") { comm[key] = value.clone(); }
        let data = if module == "music.musicasset.PlaylistDetailWrite" && param["dirId"] == 201 {
            let payload =
                json!({"comm":comm,"req":{"module":module,"method":method,"param":param}});
            let body = serde_json::to_vec(&payload)?;
            let cipher = Aes128Gcm::new_from_slice(&[
                189, 48, 95, 16, 208, 255, 116, 182, 239, 84, 218, 184, 53, 181, 225, 207,
            ])
            .expect("AES key length");
            let random = uuid::Uuid::new_v4();
            let iv = &random.as_bytes()[..12];
            let encrypted = cipher
                .encrypt(Nonce::from_slice(iv), body.as_ref())
                .map_err(|_| AppError::new("REQUEST_ENCRYPTION", "QQ 请求加密失败。"))?;
            let wire = base64::engine::general_purpose::STANDARD
                .encode([iv, encrypted.as_slice()].concat());
            let response = self
                .client
                .post("https://u6.y.qq.com/cgi-bin/musics.fcg")
                .query(&[("encoding", "ag-1"), ("sign", &web_sign(&body))])
                .header("Cookie", auth.cookie_header())
                .header("Content-Type", "text/plain")
                .header("Accept", "application/octet-stream")
                .body(wire)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            let key = [
                122, 63, 140, 29, 94, 155, 47, 10, 108, 77, 126, 139, 31, 58, 92, 157, 14, 43, 111,
                74, 129,
            ];
            let plain: Vec<u8> = response
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ key[i % key.len()])
                .collect();
            let value: Value = serde_json::from_str(&String::from_utf8_lossy(&plain))?;
            crate::qq::check_code(&value)?;
            crate::qq::check_code(&value["req"])?;
            value["req"]
                .get("data")
                .ok_or_else(|| AppError::protocol("write data"))?
                .clone()
        } else {
            self.rpc(Some(auth), comm, module, method, param).await?
        };
        for key in ["code", "subcode"] {
            if data
                .get(key)
                .and_then(Value::as_i64)
                .is_some_and(|code| code != 0)
            {
                return Err(AppError::new(
                    "CLOUD_WRITE_FAILED",
                    "QQ 音乐拒绝了本次写入，请刷新确认实际状态。",
                ));
            }
        }
        match data.get("retCode").and_then(Value::as_i64) {
            Some(0) => Ok(data),
            None if method == "EditPlaylist" => Ok(data),
            code => Err(AppError::new(
                "CLOUD_WRITE_FAILED",
                format!(
                    "QQ 音乐未确认操作成功（{}），请刷新查看实际结果。",
                    code.map_or("响应不完整".into(), |n| n.to_string())
                ),
            )),
        }
    }
    async fn owned_dir(&self, auth: &Credentials, id: &str) -> Result<u64> {
        let playlist = self
            .library(auth)
            .await?
            .playlists
            .into_iter()
            .find(|p| p.id == id && p.kind != "collected")
            .ok_or_else(|| AppError::new("READ_ONLY_PLAYLIST", "只能修改当前账户自己的歌单。"))?;
        playlist
            .dir_id
            .ok_or_else(|| AppError::protocol("playlist.dirid"))
    }
    pub async fn create_playlist(&self, auth: &Credentials, name: &str) -> Result<Library> {
        validate_name(name)?;
        let result = self
            .write_playlist(
                auth,
                "music.musicasset.PlaylistBaseWrite",
                "AddPlaylist",
                json!({"dirName":name.trim()}),
            )
            .await?;
        let id = result
            .pointer("/result/dirId")
            .and_then(Value::as_u64)
            .ok_or_else(|| AppError::protocol("created dirId"))?;
        let library = self.library(auth).await?;
        if !library.playlists.iter().any(|p| p.dir_id == Some(id)) {
            return Err(AppError::new(
                "CLOUD_VERIFY_FAILED",
                "QQ 已接收创建请求，但刷新尚未确认新歌单，请稍后刷新。",
            ));
        }
        Ok(library)
    }
    pub async fn delete_playlist(&self, auth: &Credentials, id: &str) -> Result<Library> {
        let dir = self.owned_dir(auth, id).await?;
        if dir == 201 {
            return Err(AppError::new("PROTECTED_PLAYLIST", "不能删除“我喜欢”。"));
        }
        self.write_playlist(
            auth,
            "music.musicasset.PlaylistBaseWrite",
            "DelPlaylist",
            json!({"dirId":dir}),
        )
        .await?;
        let library = self.library(auth).await?;
        if library.playlists.iter().any(|p| p.id == id) {
            return Err(AppError::new(
                "CLOUD_VERIFY_FAILED",
                "删除请求已发送，但 QQ 仍返回此歌单，请刷新确认。",
            ));
        }
        Ok(library)
    }
    pub async fn rename_playlist(
        &self,
        auth: &Credentials,
        id: &str,
        name: &str,
    ) -> Result<Library> {
        validate_name(name)?;
        let dir = self.owned_dir(auth, id).await?;
        if dir == 201 {
            return Err(AppError::new("PROTECTED_PLAYLIST", "不能重命名“我喜欢”。"));
        }
        let tid = id
            .parse::<u64>()
            .map_err(|_| AppError::protocol("playlist id"))?;
        let data = self.rpc(Some(auth),auth.comm(),"music.srfDissInfo.DissInfo","CgiGetDiss",json!({"disstid":tid,"dirid":dir,"song_begin":0,"song_num":1,"userinfo":true,"tag":true})).await?;
        let info = &data["dirinfo"];
        let required = |key: &str| {
            info[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| AppError::protocol(key))
        };
        let tags = info["vec_tagid"]
            .as_array()
            .ok_or_else(|| AppError::protocol("vec_tagid"))?
            .iter()
            .map(|v| {
                v.as_u64()
                    .map(|n| n.to_string())
                    .ok_or_else(|| AppError::protocol("tag id"))
            })
            .collect::<Result<Vec<_>>>()?
            .join(",");
        self.write_playlist(auth,"music.musicasset.PlaylistBaseWrite","EditPlaylist",json!({"dirId":dir,"mask":95,"dirNewName":name.trim(),"dirNewDesc":required("desc")?,"dirNewPicUrl":required("picurl")?,"dirNewPicUrl2":required("picurl2")?,"dirNewtaglist":tags,"dirShow":info["dir_show"].as_u64().ok_or_else(||AppError::protocol("dir_show"))?})).await?;
        let library = self.library(auth).await?;
        if !library
            .playlists
            .iter()
            .any(|p| p.id == id && p.name == name.trim())
        {
            return Err(AppError::new(
                "CLOUD_VERIFY_FAILED",
                "重命名请求已发送，刷新后尚未确认新名称。",
            ));
        }
        Ok(library)
    }
    pub async fn change_playlist_track(
        &self,
        auth: &Credentials,
        id: &str,
        track: &Track,
        add: bool,
    ) -> Result<Library> {
        crate::qq::validate_track(track)?;
        let dir = self.owned_dir(auth, id).await?;
        let detail = self
            .rpc(
                Some(auth),
                auth.comm(),
                "music.pf_song_detail_svr",
                "get_song_detail_yqq",
                json!({"song_mid":track.mid}),
            )
            .await?;
        let info = &detail["track_info"];
        if info["id"].as_u64() != Some(track.id) || info["mid"].as_str() != Some(&track.mid) {
            return Err(AppError::new(
                "TRACK_CHANGED",
                "歌曲身份无法确认，未修改歌单。",
            ));
        }
        let kind = info["type"]
            .as_u64()
            .ok_or_else(|| AppError::protocol("song type"))?;
        let mut param = json!({"dirId":dir,"v_songInfo":[{"songId":track.id,"songType":kind}]});
        if dir != 201 {
            param["tid"] = json!(id
                .parse::<u64>()
                .map_err(|_| AppError::protocol("playlist id"))?);
            param["bFmtUtf8"] = json!(true);
        }
        self.write_playlist(
            auth,
            "music.musicasset.PlaylistDetailWrite",
            if add { "AddSonglist" } else { "DelSonglist" },
            param,
        )
        .await?;
        let mut offset = 0;
        let present = loop {
            let page = self.playlist(auth, id, offset).await?;
            if page.tracks.iter().any(|t| t.mid == track.mid) {
                break true;
            }
            if !page.has_more {
                break false;
            }
            offset = page.next_offset;
        };
        if present != add {
            return Err(AppError::new(
                "CLOUD_VERIFY_FAILED",
                "QQ 已接收请求，但读回未确认变化，请刷新后再决定是否重试。",
            ));
        }
        self.library(auth).await
    }
}
fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.trim().chars().count() > 100 {
        return Err(AppError::new("INVALID_NAME", "歌单名称需要 1–100 个字符。"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn web_signature_matches_an_independent_sha1_fixture() {
        assert_eq!(
            web_sign(b"{}"),
            "zzcf8e26805gyafigxmxjehoe02mvsjjgtwzw6f1a05f9"
        );
    }
    #[test]
    fn rejects_empty_or_oversized_cloud_names() {
        assert!(validate_name("  ").is_err());
        assert!(validate_name(&"歌".repeat(101)).is_err());
        assert!(validate_name(" 我的歌单 ").is_ok());
    }
}
