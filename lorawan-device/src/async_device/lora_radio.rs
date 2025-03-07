use super::radio::{PhyRxTx, RfConfig, RxQuality, TxConfig};
use super::Timings;

use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::{DelayNs, LoRa};

/// LoRa radio using the physical layer API in the external lora-phy crate.
///
/// The const generic P is the max power the radio may be instructed to transmit at. The const
/// generic G is the antenna gain and board loss in dBi.
pub struct LoRaRadio<RK, DLY, const P: u8, const G: i8 = 0>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    pub(crate) lora: LoRa<RK, DLY>,
    rx_pkt_params: Option<lora_phy::mod_params::PacketParams>,
}
impl<RK, DLY, const P: u8, const G: i8> LoRaRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    pub fn new(lora: LoRa<RK, DLY>) -> Self {
        Self { lora, rx_pkt_params: None }
    }
}

/// Provide the timing values for boards supported by the external lora-phy crate
impl<RK, DLY, const P: u8, const G: i8> Timings for LoRaRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    // TODO: Use a struct with Default or Option fallback?
    fn get_rx_window_offset_ms(&self) -> i32 {
        -50
    }
    fn get_rx_window_duration_ms(&self) -> u32 {
        1050
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug)]
pub enum Error {
    Radio(RadioError),
    NoRxParams,
}

impl From<RadioError> for Error {
    fn from(err: RadioError) -> Self {
        Error::Radio(err)
    }
}

/// Provide the LoRa physical layer rx/tx interface for boards supported by the external lora-phy
/// crate
impl<RK, DLY, const P: u8, const G: i8> PhyRxTx for LoRaRadio<RK, DLY, P, G>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    type PhyError = Error;

    const ANTENNA_GAIN: i8 = G;

    const MAX_RADIO_POWER: u8 = P;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.rf.bb.sf,
            config.rf.bb.bw,
            config.rf.bb.cr,
            config.rf.frequency,
        )?;
        let mut tx_pkt_params =
            self.lora.create_tx_packet_params(8, false, true, false, &mdltn_params)?;

        // TODO: 3rd argument (boost_if_possible) shouldn't be exposed, as it depends
        // on physical board layout. Needs to be eventually handled from lora-phy side.
        self.lora.prepare_for_tx(&mdltn_params, config.pw.into(), false).await?;
        self.lora.tx(&mdltn_params, &mut tx_pkt_params, buffer, 0xffffff).await?;
        Ok(0)
    }

    async fn setup_rx(&mut self, config: RfConfig) -> Result<(), Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.bb.sf,
            config.bb.bw,
            config.bb.cr,
            config.frequency,
        )?;
        let rx_pkt_params =
            self.lora.create_rx_packet_params(8, false, 255, true, true, &mdltn_params)?;
        self.lora.prepare_for_rx(&mdltn_params, &rx_pkt_params, None, None, false).await?;
        self.rx_pkt_params = Some(rx_pkt_params);
        Ok(())
    }

    async fn rx(
        &mut self,
        receiving_buffer: &mut [u8],
    ) -> Result<(usize, RxQuality), Self::PhyError> {
        if let Some(rx_params) = &self.rx_pkt_params {
            match self.lora.rx(rx_params, receiving_buffer).await {
                Ok((received_len, rx_pkt_status)) => {
                    Ok((
                        received_len as usize,
                        RxQuality::new(rx_pkt_status.rssi, rx_pkt_status.snr as i8), // downcast snr
                    ))
                }
                Err(err) => Err(err.into()),
            }
        } else {
            Err(Error::NoRxParams)
        }
    }
}
