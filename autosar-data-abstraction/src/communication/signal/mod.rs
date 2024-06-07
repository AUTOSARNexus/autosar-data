use crate::{abstraction_element, element_iterator, make_unique_name, AbstractionElement, ArPackage, AutosarAbstractionError, EcuInstance};
use autosar_data::{AutosarDataError, Element, ElementName, EnumItem};

use super::{CommunicationDirection, PhysicalChannel};

/// The [`Signal`] represents the combination of an `I-SIGNAL` and its paired `SYSTEM-SIGNAL`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Signal(Element);
abstraction_element!(Signal, ISignal);

impl Signal {
    pub(crate) fn new(
        name: &str,
        bit_length: u64,
        sig_package: &ArPackage,
        sys_package: &ArPackage,
    ) -> Result<Self, AutosarAbstractionError> {
        if sig_package == sys_package {
            return Err(AutosarAbstractionError::InvalidParameter(
                "you must use different packages for the ISIgnal and the SystemSignal".to_string(),
            ));
        }
        let sig_pkg_elements = sig_package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_isignal = sig_pkg_elements.create_named_sub_element(ElementName::ISignal, name)?;

        let sys_pkg_elements = sys_package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_syssignal = sys_pkg_elements.create_named_sub_element(ElementName::SystemSignal, name)?;

        elem_isignal
            .create_sub_element(ElementName::Length)?
            .set_character_data(bit_length.to_string())?;
        elem_isignal
            .create_sub_element(ElementName::SystemSignalRef)?
            .set_reference_target(&elem_syssignal)?;
        elem_isignal
            .create_sub_element(ElementName::DataTypePolicy)?
            .set_character_data(EnumItem::Override)?;

        Ok(Self(elem_isignal))
    }

    pub fn set_datatype(&self, _datatype: ()) -> Result<(), AutosarAbstractionError> {
        todo!()
    }

    pub fn set_transformation(&self) -> Result<(), AutosarAbstractionError> {
        todo!()
    }
}

//##################################################################

/// The [`SignalGroup`] represents the combination of an `I-SIGNAL-GROUP` and its paired `SYSTEM-SIGNAL-GROUP`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignalGroup(Element);
abstraction_element!(SignalGroup, ISignalGroup);

impl SignalGroup {
    pub(crate) fn new(
        name: &str,
        sig_package: &ArPackage,
        sys_package: &ArPackage,
    ) -> Result<Self, AutosarAbstractionError> {
        if sig_package == sys_package {
            return Err(AutosarAbstractionError::InvalidParameter(
                "you must use different packages for the ISIgnal and the SystemSignal".to_string(),
            ));
        }

        let sig_pkg_elements = sig_package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_isiggrp = sig_pkg_elements.create_named_sub_element(ElementName::ISignalGroup, name)?;
        let sys_pkg_elements = sys_package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_syssiggrp = sys_pkg_elements.create_named_sub_element(ElementName::SystemSignalGroup, name)?;

        elem_isiggrp
            .create_sub_element(ElementName::SystemSignalGroupRef)?
            .set_reference_target(&elem_syssiggrp)?;

        Ok(Self(elem_isiggrp))
    }

    /// Add a signal to the signal group
    pub fn add_signal(&self, _signal: &Signal) -> Result<(), AutosarAbstractionError> {
        todo!()
    }

    /// Iterator over all [`Signal`]s in this group
    ///
    /// # Example
    pub fn signals(&self) -> SignalsIterator {
        SignalsIterator::new(self.element().get_sub_element(ElementName::Signals))
    }
}

//##################################################################


/// an ISignalTriggering triggers a signal in a PDU
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ISignalTriggering(Element);
abstraction_element!(ISignalTriggering, ISignalTriggering);

impl ISignalTriggering {
    pub(crate) fn new(signal: &Signal, channel: &PhysicalChannel) -> Result<Self, AutosarAbstractionError> {
        let model = channel.element().model()?;
        let base_path = channel.element().path()?;
        let signal_name = signal
            .name()
            .ok_or(AutosarAbstractionError::InvalidParameter("invalid signal".to_string()))?;
        let pt_name = format!("ST_{signal_name}");
        let pt_name = make_unique_name(&model, base_path, pt_name);

        let triggerings = channel
            .element()
            .get_or_create_sub_element(ElementName::ISignalTriggerings)?;
        let st_elem = triggerings.create_named_sub_element(ElementName::ISignalTriggering, &pt_name)?;
        st_elem
            .create_sub_element(ElementName::ISignalRef)?
            .set_reference_target(signal.element())?;

        let pt = Self(st_elem);

        Ok(pt)
    }

    /// get the physical channel that contains this signal triggering
    pub fn physical_channel(&self) -> Result<PhysicalChannel, AutosarAbstractionError> {
        let channel_elem = self.element().named_parent()?.ok_or(AutosarDataError::ItemDeleted)?;
        PhysicalChannel::try_from(channel_elem)
    }

    pub fn connect_to_ecu(&self, ecu: &EcuInstance, direction: CommunicationDirection) -> Result<ISignalPort, AutosarAbstractionError> {
        for signal_port in self.signal_ports() {
            if let (Some(existing_ecu), Some(existing_direction)) = (signal_port.ecu(), signal_port.communication_direction())
            {
                if existing_ecu == *ecu && existing_direction == direction {
                    return Ok(signal_port);
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
        let sp_elem = connector
            .get_or_create_sub_element(ElementName::EcuCommPortInstances)?
            .create_named_sub_element(ElementName::ISignalPort, &port_name)?;
        sp_elem
            .create_sub_element(ElementName::CommunicationDirection)?
            .set_character_data::<EnumItem>(direction.into())?;

        self.element()
            .get_or_create_sub_element(ElementName::ISignalPortRefs)?
            .create_sub_element(ElementName::ISignalPortRef)?
            .set_reference_target(&sp_elem)?;

        Ok(ISignalPort(sp_elem))
    }
    
    pub fn signal_ports(&self) -> ISignalPortIterator {
        ISignalPortIterator::new(self.element().get_sub_element(ElementName::ISignalPortRefs))
    }
}

//##################################################################

/// The IPduPort allows an ECU to send or receive a PDU
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ISignalPort(Element);
abstraction_element!(ISignalPort, ISignalPort);

impl ISignalPort {
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
pub enum TransferProperty {
    Pending,
    Triggered,
    TriggeredOnChange,
    TriggeredOnChangeWithoutRepetition,
    TriggeredWithoutRepetition,
}

impl From<TransferProperty> for EnumItem {
    fn from(value: TransferProperty) -> Self {
        match value {
            TransferProperty::Pending => EnumItem::Pending,
            TransferProperty::Triggered => EnumItem::Triggered,
            TransferProperty::TriggeredOnChange => EnumItem::TriggeredOnChange,
            TransferProperty::TriggeredOnChangeWithoutRepetition => EnumItem::TriggeredOnChangeWithoutRepetition,
            TransferProperty::TriggeredWithoutRepetition => EnumItem::TriggeredWithoutRepetition,
        }
    }
}

impl TryFrom<EnumItem> for TransferProperty {
    type Error = AutosarAbstractionError;

    fn try_from(value: EnumItem) -> Result<Self, Self::Error> {
        match value {
            EnumItem::Pending => Ok(TransferProperty::Pending),
            EnumItem::Triggered => Ok(TransferProperty::Triggered),
            EnumItem::TriggeredOnChange => Ok(TransferProperty::TriggeredOnChange),
            EnumItem::TriggeredOnChangeWithoutRepetition => Ok(TransferProperty::TriggeredOnChangeWithoutRepetition),
            EnumItem::TriggeredWithoutRepetition => Ok(TransferProperty::TriggeredWithoutRepetition),
            _ => Err(AutosarAbstractionError::ValueConversionError {
                value: value.to_string(),
                dest: "TransferProperty".to_string(),
            }),
        }
    }
}

//##################################################################

element_iterator!(
    ISignalPortIterator,
    ISignalPort,
    (|element: Element| element.get_reference_target().ok())
);

//##################################################################

element_iterator!(
    SignalsIterator,
    Signal,
    (|element: Element| element.get_reference_target().ok())
);
