use crate::communication::{CommunicationDirection, ISignalTriggering, PhysicalChannel, Signal, TransferProperty};
use crate::{
    abstraction_element, element_iterator, make_unique_name, reflist_iterator, AbstractionElement, ArPackage,
    AutosarAbstractionError, ByteOrder, EcuInstance,
};
use autosar_data::{AutosarDataError, Element, ElementName, EnumItem};

//##################################################################

/// Represents the IPdus handled by Com
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ISignalIPdu(Element);
abstraction_element!(ISignalIPdu, ISignalIPdu);

impl ISignalIPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::ISignalIPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<ISignalIPdu> for Pdu {
    fn from(value: ISignalIPdu) -> Self {
        Pdu::ISignalIPdu(value)
    }
}

impl ISignalIPdu {
    /// returns an iterator over all signals mapped to the PDU
    pub fn mapped_signals(&self) -> ISIgnalToIPduMappingsIterator {
        ISIgnalToIPduMappingsIterator::new(self.element().get_sub_element(ElementName::ISignalToPduMappings))
    }

    pub fn map_signal(
        &self,
        signal: &Signal,
        start_position: u32,
        byte_order: ByteOrder,
        update_bit: Option<u32>,
        transfer_property: TransferProperty,
    ) -> Result<ISignalToIPduMapping, AutosarAbstractionError> {
        let signal_name = signal
            .name()
            .ok_or(AutosarAbstractionError::InvalidParameter("invalid signal".to_string()))?;
        // for mapping in self.mapped_signals() {
        //     todo? check if the new signal overlaps any existing ones
        // }

        // add a pdu triggering for the newly mapped PDU to each frame triggering of this frame
        for pt in self.pdu_triggerings() {
            let st = pt.add_signal_triggering(&signal)?;
            for pdu_port in pt.pdu_ports() {
                if let (Some(ecu), Some(direction)) = (pdu_port.ecu(), pdu_port.communication_direction()) {
                    st.connect_to_ecu(&ecu, direction)?;
                }
            }
        }

        // create and return the new mapping
        let model = self.element().model()?;
        let base_path = self.element().path()?;
        let name = make_unique_name(&model, base_path, signal_name);

        let mappings = self
            .element()
            .get_or_create_sub_element(ElementName::ISignalToPduMappings)?;

        ISignalToIPduMapping::new(
            &name,
            &mappings,
            &signal,
            start_position,
            byte_order,
            update_bit,
            transfer_property,
        )
    }

    pub fn pdu_triggerings(&self) -> PduTriggeringsIterator {
        let model_result = self.element().model();
        let path_result = self.element().path();
        if let (Ok(model), Ok(path)) = (model_result, path_result) {
            let reflist = model.get_references_to(&path);
            PduTriggeringsIterator::new(reflist)
        } else {
            PduTriggeringsIterator::new(vec![])
        }
    }
}

//##################################################################

/// ISignalToIPduMapping connects an isignal to an isignalipdu
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ISignalToIPduMapping(Element);
abstraction_element!(ISignalToIPduMapping, ISignalToIPduMapping);

impl ISignalToIPduMapping {
    fn new(
        name: &str,
        mappings: &Element,
        signal: &Signal,
        start_position: u32,
        byte_order: ByteOrder,
        update_bit: Option<u32>,
        transfer_property: TransferProperty,
    ) -> Result<Self, AutosarAbstractionError> {
        let signal_mapping = mappings.create_named_sub_element(ElementName::ISignalToIPduMapping, name)?;
        signal_mapping
            .create_sub_element(ElementName::ISignalRef)?
            .set_reference_target(signal.element())?;
        signal_mapping
            .create_sub_element(ElementName::PackingByteOrder)?
            .set_character_data::<EnumItem>(byte_order.into())?;
        signal_mapping
            .create_sub_element(ElementName::StartPosition)?
            .set_character_data(start_position as u64)?;
        signal_mapping
            .create_sub_element(ElementName::TransferProperty)?
            .set_character_data::<EnumItem>(transfer_property.into())?;
        if let Some(update_bit_pos) = update_bit {
            signal_mapping
                .create_sub_element(ElementName::UpdateIndicationBitPosition)?
                .set_character_data(update_bit_pos as u64)?;
        }

        Ok(Self(signal_mapping))
    }

    /// Reference to the Signal that is mapped into the PDU. The signal reference is mandatory.
    pub fn signal(&self) -> Option<Signal> {
        self.element()
            .get_sub_element(ElementName::ISignalRef)
            .and_then(|sigref| sigref.get_reference_target().ok())
            .and_then(|signal_elem| Signal::try_from(signal_elem).ok())
    }

    /// Byte order of the data in the signal.
    pub fn byte_order(&self) -> Option<ByteOrder> {
        self.element()
            .get_sub_element(ElementName::PackingByteOrder)
            .and_then(|pbo| pbo.character_data())
            .and_then(|cdata| cdata.enum_value())
            .and_then(|enumval| enumval.try_into().ok())
    }

    /// Start position of the signal data within the PDU (bit position). The start position is mandatory.
    pub fn start_position(&self) -> Option<u32> {
        self.element()
            .get_sub_element(ElementName::StartPosition)
            .and_then(|pbo| pbo.character_data())
            .and_then(|cdata| cdata.decode_integer())
    }

    /// Bit position of the update bit for the mapped signal. Not all signals use an update bit.
    pub fn update_bit(&self) -> Option<u32> {
        self.element()
            .get_sub_element(ElementName::StartPosition)
            .and_then(|pbo| pbo.character_data())
            .and_then(|cdata| cdata.decode_integer())
    }

    /// Bit position of the update bit for the mapped signal. Not all signals use an update bit.
    pub fn transfer_property(&self) -> Option<TransferProperty> {
        self.element()
            .get_sub_element(ElementName::TransferProperty)
            .and_then(|pbo| pbo.character_data())
            .and_then(|cdata| cdata.enum_value())
            .and_then(|enumval| enumval.try_into().ok())
    }
}

//##################################################################

/// Network Management Pdu
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NmPdu(Element);
abstraction_element!(NmPdu, NmPdu);

impl NmPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::NmPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<NmPdu> for Pdu {
    fn from(value: NmPdu) -> Self {
        Pdu::NmPdu(value)
    }
}

//##################################################################

/// This is a Pdu of the transport layer. The main purpose of the TP layer is to segment and reassemble IPdus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NPdu(Element);
abstraction_element!(NPdu, NPdu);

impl NPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::NPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<NPdu> for Pdu {
    fn from(value: NPdu) -> Self {
        Pdu::NPdu(value)
    }
}

//##################################################################

/// Represents the IPdus handled by Dcm
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DcmIPdu(Element);
abstraction_element!(DcmIPdu, DcmIPdu);

impl DcmIPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::DcmIPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<DcmIPdu> for Pdu {
    fn from(value: DcmIPdu) -> Self {
        Pdu::DcmIPdu(value)
    }
}

//##################################################################

/// This element is used for AUTOSAR Pdus without additional attributes that are routed by a bus interface
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GeneralPurposePdu(Element);
abstraction_element!(GeneralPurposePdu, GeneralPurposePdu);

impl GeneralPurposePdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::GeneralPurposePdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<GeneralPurposePdu> for Pdu {
    fn from(value: GeneralPurposePdu) -> Self {
        Pdu::GeneralPurposePdu(value)
    }
}

//##################################################################

/// This element is used for AUTOSAR Pdus without attributes that are routed by the PduR
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GeneralPurposeIPdu(Element);
abstraction_element!(GeneralPurposeIPdu, GeneralPurposeIPdu);

impl GeneralPurposeIPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::GeneralPurposeIPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<GeneralPurposeIPdu> for Pdu {
    fn from(value: GeneralPurposeIPdu) -> Self {
        Pdu::GeneralPurposeIPdu(value)
    }
}

//##################################################################

/// Several IPdus can be collected in one ContainerIPdu based on the headerType
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContainerIPdu(Element);
abstraction_element!(ContainerIPdu, ContainerIPdu);

impl ContainerIPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::ContainerIPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<ContainerIPdu> for Pdu {
    fn from(value: ContainerIPdu) -> Self {
        Pdu::ContainerIPdu(value)
    }
}

//##################################################################

/// Wraps an IPdu to protect it from unauthorized manipulation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecuredIPdu(Element);
abstraction_element!(SecuredIPdu, SecuredIPdu);

impl SecuredIPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::SecuredIPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<SecuredIPdu> for Pdu {
    fn from(value: SecuredIPdu) -> Self {
        Pdu::SecuredIPdu(value)
    }
}

//##################################################################

/// The multiplexed pdu contains one of serveral signal pdus
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MultiplexedIPdu(Element);
abstraction_element!(MultiplexedIPdu, MultiplexedIPdu);

impl MultiplexedIPdu {
    pub(crate) fn new(name: &str, package: &ArPackage, length: u32) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_pdu = pkg_elements.create_named_sub_element(ElementName::MultiplexedIPdu, name)?;
        elem_pdu
            .create_sub_element(ElementName::Length)?
            .set_character_data(length.to_string())?;

        Ok(Self(elem_pdu))
    }
}

impl From<MultiplexedIPdu> for Pdu {
    fn from(value: MultiplexedIPdu) -> Self {
        Pdu::MultiplexedIPdu(value)
    }
}

//##################################################################

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pdu {
    ISignalIPdu(ISignalIPdu),
    NmPdu(NmPdu),
    NPdu(NPdu),
    DcmIPdu(DcmIPdu),
    GeneralPurposePdu(GeneralPurposePdu),
    GeneralPurposeIPdu(GeneralPurposeIPdu),
    ContainerIPdu(ContainerIPdu),
    SecuredIPdu(SecuredIPdu),
    MultiplexedIPdu(MultiplexedIPdu),
}

impl AbstractionElement for Pdu {
    fn element(&self) -> &Element {
        match self {
            Pdu::ISignalIPdu(pdu) => pdu.element(),
            Pdu::NmPdu(pdu) => pdu.element(),
            Pdu::NPdu(pdu) => pdu.element(),
            Pdu::DcmIPdu(pdu) => pdu.element(),
            Pdu::GeneralPurposePdu(pdu) => pdu.element(),
            Pdu::GeneralPurposeIPdu(pdu) => pdu.element(),
            Pdu::ContainerIPdu(pdu) => pdu.element(),
            Pdu::SecuredIPdu(pdu) => pdu.element(),
            Pdu::MultiplexedIPdu(pdu) => pdu.element(),
        }
    }
}

impl TryFrom<Element> for Pdu {
    type Error = AutosarAbstractionError;

    fn try_from(element: Element) -> Result<Self, Self::Error> {
        match element.element_name() {
            ElementName::ISignalIPdu => Ok(ISignalIPdu::try_from(element)?.into()),
            ElementName::NmPdu => Ok(NmPdu::try_from(element)?.into()),
            ElementName::NPdu => Ok(NPdu::try_from(element)?.into()),
            ElementName::DcmIPdu => Ok(DcmIPdu::try_from(element)?.into()),
            ElementName::GeneralPurposePdu => Ok(GeneralPurposePdu::try_from(element)?.into()),
            ElementName::GeneralPurposeIPdu => Ok(GeneralPurposeIPdu::try_from(element)?.into()),
            ElementName::ContainerIPdu => Ok(ContainerIPdu::try_from(element)?.into()),
            ElementName::SecuredIPdu => Ok(SecuredIPdu::try_from(element)?.into()),
            ElementName::MultiplexedIPdu => Ok(MultiplexedIPdu::try_from(element)?.into()),
            _ => Err(AutosarAbstractionError::ConversionError {
                element,
                dest: "Pdu".to_string(),
            }),
        }
    }
}

//##################################################################

/// a PduTriggering triggers a PDU in a frame or ethernet connection
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PduTriggering(Element);
abstraction_element!(PduTriggering, PduTriggering);

impl PduTriggering {
    pub(crate) fn new(pdu: &Pdu, channel: &PhysicalChannel) -> Result<Self, AutosarAbstractionError> {
        let model = channel.element().model()?;
        let base_path = channel.element().path()?;
        let pdu_name = pdu
            .name()
            .ok_or(AutosarAbstractionError::InvalidParameter("invalid pdu".to_string()))?;
        let pt_name = format!("PT_{pdu_name}");
        let pt_name = make_unique_name(&model, base_path, pt_name);

        let triggerings = channel
            .element()
            .get_or_create_sub_element(ElementName::PduTriggerings)?;
        let pt_elem = triggerings.create_named_sub_element(ElementName::PduTriggering, &pt_name)?;
        pt_elem
            .create_sub_element(ElementName::IPduRef)?
            .set_reference_target(pdu.element())?;

        let pt = Self(pt_elem);

        if let Pdu::ISignalIPdu(isignal_ipdu) = pdu {
            for signal_mapping in isignal_ipdu.mapped_signals() {
                if let Some(signal) = signal_mapping.signal() {
                    pt.add_signal_triggering(&signal)?;
                }
            }
        }

        Ok(pt)
    }

    /// get the physical channel that contains this pdu triggering
    pub fn physical_channel(&self) -> Result<PhysicalChannel, AutosarAbstractionError> {
        let channel_elem = self.element().named_parent()?.ok_or(AutosarDataError::ItemDeleted)?;
        PhysicalChannel::try_from(channel_elem)
    }

    /// create an IPduPort to connect a PduTriggering to an EcuInstance
    pub fn connect_to_ecu(
        &self,
        ecu: &EcuInstance,
        direction: CommunicationDirection,
    ) -> Result<IPduPort, AutosarAbstractionError> {
        for pdu_port in self.pdu_ports() {
            if let (Some(existing_ecu), Some(existing_direction)) = (pdu_port.ecu(), pdu_port.communication_direction())
            {
                if existing_ecu == *ecu && existing_direction == direction {
                    return Ok(pdu_port);
                }
            }
        }

        let channel = self.physical_channel()?;
        let connector = channel
            .get_ecu_connector(ecu)
            .ok_or(AutosarAbstractionError::InvalidParameter(
                "The ECU is not connected to the channel".to_string(),
            ))?;

        let name = self.name().ok_or(AutosarDataError::ItemDeleted)?;
        let suffix = match direction {
            CommunicationDirection::In => "Rx",
            CommunicationDirection::Out => "Tx",
        };
        let port_name = format!("{name}_{suffix}",);
        let pp_elem = connector
            .get_or_create_sub_element(ElementName::EcuCommPortInstances)?
            .create_named_sub_element(ElementName::IPduPort, &port_name)?;
        pp_elem
            .create_sub_element(ElementName::CommunicationDirection)?
            .set_character_data::<EnumItem>(direction.into())?;

        self.element()
            .get_or_create_sub_element(ElementName::IPduPortRefs)?
            .create_sub_element(ElementName::IPduPortRef)?
            .set_reference_target(&pp_elem)?;

        for st in self.signal_triggerings() {
            st.connect_to_ecu(ecu, direction)?;
        }

        Ok(IPduPort(pp_elem))
    }

    pub fn pdu_ports(&self) -> IPduPortIterator {
        IPduPortIterator::new(self.element().get_sub_element(ElementName::IPduPortRefs))
    }

    pub fn signal_triggerings(&self) -> PtSignalTriggeringsIterator {
        PtSignalTriggeringsIterator::new(self.element().get_sub_element(ElementName::ISignalTriggerings))
    }

    pub fn add_signal_triggering(&self, signal: &Signal) -> Result<ISignalTriggering, AutosarAbstractionError> {
        let channel = self.physical_channel()?;
        let st = ISignalTriggering::new(signal, &channel)?;
        let triggerings = self
            .element()
            .get_or_create_sub_element(ElementName::ISignalTriggerings)?;
        triggerings
            .create_sub_element(ElementName::ISignalTriggeringRefConditional)?
            .create_sub_element(ElementName::ISignalTriggeringRef)?
            .set_reference_target(st.element())?;

        for pdu_port in self.pdu_ports() {
            if let (Some(ecu), Some(direction)) = (pdu_port.ecu(), pdu_port.communication_direction()) {
                st.connect_to_ecu(&ecu, direction)?;
            }
        }

        Ok(st)
    }
}

//##################################################################

/// The IPduPort allows an ECU to send or receive a PDU
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IPduPort(Element);
abstraction_element!(IPduPort, IPduPort);

impl IPduPort {
    pub fn ecu(&self) -> Option<EcuInstance> {
        let comm_connector_elem = self.element().named_parent().ok()??;
        let ecu_elem = comm_connector_elem.named_parent().ok()??;
        EcuInstance::try_from(ecu_elem).ok()
    }

    pub fn communication_direction(&self) -> Option<CommunicationDirection> {
        self.element()
            .get_sub_element(ElementName::CommunicationDirection)?
            .character_data()?
            .enum_value()?
            .try_into()
            .ok()
    }
}

//##################################################################

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PduCollectionTrigger {
    Always,
    Never,
}

impl From<PduCollectionTrigger> for EnumItem {
    fn from(value: PduCollectionTrigger) -> Self {
        match value {
            PduCollectionTrigger::Always => EnumItem::Always,
            PduCollectionTrigger::Never => EnumItem::Never,
        }
    }
}

//##################################################################

element_iterator!(ISIgnalToIPduMappingsIterator, ISignalToIPduMapping, Some);

//##################################################################

element_iterator!(
    IPduPortIterator,
    IPduPort,
    (|element: Element| element.get_reference_target().ok())
);

//##################################################################

reflist_iterator!(PduTriggeringsIterator, PduTriggering);

//##################################################################

element_iterator!(
    PtSignalTriggeringsIterator,
    ISignalTriggering,
    (|element: Element| element
        .get_sub_element(ElementName::ISignalTriggeringRef)
        .and_then(|str| str.get_reference_target().ok()))
);
