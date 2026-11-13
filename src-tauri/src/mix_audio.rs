use crate::{
    error::{AppError, Result},
    main_window,
    models::Track,
    qq::{validate_track, QqMusic},
    AppState,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{State, WebviewWindow};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QqMixPlan {
    out: f64,
    input: f64,
    duration: f64,
    rate: f64,
    mode: String,
}

#[derive(Deserialize)]
struct QqMixCues {
    #[serde(rename = "OutCueCuts")]
    cuts: Vec<f64>,
    #[serde(rename = "InCueEntrys")]
    entries: Vec<f64>,
    #[serde(rename = "OutBpm")]
    out_bpm: f64,
    #[serde(rename = "InBpm")]
    in_bpm: f64,
    #[serde(rename = "MixMode")]
    mode: String,
    #[serde(rename = "type")]
    kind: u8,
}

impl QqMixCues {
    fn into_plan(self) -> Result<QqMixPlan> {
        let invalid = || AppError::new("INVALID_QQ_MIX", "QQ 音乐返回的衔接点无法使用。");
        if self.mode != "amfilter2" || self.kind > 3 {
            return Err(AppError::new(
                "UNSUPPORTED_QQ_MIX",
                "QQ 音乐返回了当前无法播放的衔接模式。",
            ));
        }
        let cut = self
            .cuts
            .into_iter()
            .find(|cut| *cut > 0.0)
            .ok_or_else(invalid)?;
        let [entry, input, ..] = self.entries.as_slice() else {
            return Err(invalid());
        };
        if *input < 0.0 || *entry <= *input || self.out_bpm <= 0.0 || self.in_bpm <= 0.0 {
            return Err(invalid());
        }
        // Match the beat subdivision used by QQ's amfilter2 strategy before
        // aligning the incoming entry with the outgoing cut.
        let relative_gap = |bpm: f64| (bpm - self.in_bpm).abs() / bpm;
        let beat_scale = [1.0 / 3.0, 0.5, 1.0, 2.0, 3.0]
            .into_iter()
            .min_by(|a, b| {
                relative_gap(self.out_bpm * a).total_cmp(&relative_gap(self.out_bpm * b))
            })
            .expect("five beat subdivisions");
        let rate = (self.out_bpm * beat_scale) / self.in_bpm;
        let duration = (entry - input) / rate;
        let out = cut - duration;
        if !(0.5..=2.0).contains(&rate) || !out.is_finite() || out < 0.0 {
            return Err(invalid());
        }
        Ok(QqMixPlan {
            out,
            input: *input,
            duration,
            rate,
            mode: self.mode,
        })
    }
}

impl QqMusic {
    pub(crate) async fn mix_plan(&self, from: &str, to: &str) -> Result<QqMixPlan> {
        let data = self.rpc(
            None,
            json!({"ct":24,"cv":4747474,"uin":0,"format":"json","platform":"yqq.json","needNewCode":1}),
            "music.mir.MixPlanSvr",
            "Build",
            json!({"songMids":[from,to]}),
        ).await?;
        let plan = data["mixPlan"]
            .as_str()
            .ok_or_else(|| AppError::protocol("mixPlan"))?;
        serde_json::from_str::<QqMixCues>(plan)?.into_plan()
    }
}

#[tauri::command]
pub async fn get_qq_mix_plan(
    window: WebviewWindow,
    state: State<'_, AppState>,
    from: Track,
    to: Track,
) -> Result<QqMixPlan> {
    main_window(&window)?;
    let snapshot = state.snapshot()?;
    validate_track(&from)?;
    validate_track(&to)?;
    let plan = state.qq.mix_plan(&from.mid, &to.mid).await?;
    state.unchanged(&snapshot)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_qq_entry_with_outgoing_cut_and_rejects_missing_cues() {
        let payload = json!({
            "OutCueCuts": [293.27, -1], "InCueEntrys": [9.07, 0.54, -1, -1],
            "OutBpm": 96.7742, "InBpm": 75.0, "MixMode": "amfilter2", "type": 3,
        });
        let plan = serde_json::from_value::<QqMixCues>(payload.clone())
            .unwrap()
            .into_plan()
            .unwrap();
        assert!((plan.rate - 1.2903226667).abs() < 1e-8);
        assert!((plan.out + plan.duration - 293.27).abs() < 1e-8);
        assert!((plan.input + plan.duration * plan.rate - 9.07).abs() < 1e-8);

        let mut missing = payload;
        missing["InCueEntrys"][1] = json!(-1);
        assert!(serde_json::from_value::<QqMixCues>(missing)
            .unwrap()
            .into_plan()
            .is_err());
    }

    #[tokio::test]
    #[ignore = "live anonymous QQ AutoMix request"]
    async fn live_qq_mix_plan_is_directional() {
        let qq = QqMusic::new().unwrap();
        let forward = qq
            .mix_plan("004X4tBC4ez4FP", "002UdGaH3dI2Cx")
            .await
            .unwrap();
        let backward = qq
            .mix_plan("002UdGaH3dI2Cx", "004X4tBC4ez4FP")
            .await
            .unwrap();
        assert!(forward.out > backward.out);
        assert!(forward.duration > 0.0 && backward.duration > 0.0);
    }
}
