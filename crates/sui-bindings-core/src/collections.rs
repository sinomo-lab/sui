use crate::application::normalized_option_name;
use std::fmt;
use sui::NotificationCenter;
use sui::NotificationId;
use sui::NotificationUrgency;
use sui::TransientNotification;
use sui::VirtualCollectionModel;

#[derive(Clone)]
pub struct BindingNotificationCenter {
    pub(crate) inner: NotificationCenter,
}

impl BindingNotificationCenter {
    pub fn new() -> Self {
        Self {
            inner: NotificationCenter::new(),
        }
    }

    pub fn notify(
        &self,
        title: impl Into<String>,
        message: impl Into<String>,
        duration: Option<f64>,
        urgency: &str,
    ) -> Result<u64, String> {
        let mut notification = TransientNotification::new(title, message);
        notification = match duration {
            Some(duration) => notification.duration(duration),
            None => notification.persistent(),
        };
        notification = notification.urgency(match normalized_option_name(urgency).as_str() {
            "polite" => NotificationUrgency::Polite,
            "assertive" | "urgent" => NotificationUrgency::Assertive,
            _ => {
                return Err(format!(
                    "notification urgency must be 'polite' or 'assertive', got '{urgency}'"
                ));
            }
        });
        Ok(self.inner.push(notification).get())
    }

    pub fn dismiss(&self, id: u64) -> bool {
        self.inner.dismiss(NotificationId::new(id))
    }

    pub fn clear(&self) -> bool {
        self.inner.clear()
    }

    pub fn len(&self) -> usize {
        self.inner.snapshot().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for BindingNotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for BindingNotificationCenter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingNotificationCenter")
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingVirtualListItem {
    pub key: u64,
    pub text: String,
}

impl BindingVirtualListItem {
    pub fn new(key: u64, text: impl Into<String>) -> Result<Self, String> {
        if key == 0 {
            return Err("virtual-list keys must be non-zero".to_string());
        }
        Ok(Self {
            key,
            text: text.into(),
        })
    }
}

#[derive(Clone)]
pub struct BindingVirtualListModel {
    pub(crate) inner: VirtualCollectionModel<u64, String>,
}

impl BindingVirtualListModel {
    pub fn new(
        name: impl Into<String>,
        items: impl IntoIterator<Item = BindingVirtualListItem>,
    ) -> Result<Self, String> {
        let name = name.into();
        VirtualCollectionModel::from_items(
            name,
            items.into_iter().map(|item| (item.key, item.text)),
        )
        .map(|inner| Self { inner })
        .map_err(|error| error.to_string())
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn append(&self, item: BindingVirtualListItem) -> Result<bool, String> {
        self.inner
            .append(item.key, item.text)
            .map_err(|error| error.to_string())
    }

    pub fn prepend(
        &self,
        items: impl IntoIterator<Item = BindingVirtualListItem>,
    ) -> Result<bool, String> {
        self.inner
            .prepend(items.into_iter().map(|item| (item.key, item.text)))
            .map_err(|error| error.to_string())
    }

    pub fn update(&self, item: BindingVirtualListItem) -> Result<bool, String> {
        self.inner
            .update(item.key, item.text)
            .map_err(|error| error.to_string())
    }

    pub fn remove(&self, key: u64) -> Result<bool, String> {
        self.inner.remove(key).map_err(|error| error.to_string())
    }

    pub fn move_to(&self, key: u64, index: usize) -> Result<bool, String> {
        self.inner
            .move_to(key, index)
            .map_err(|error| error.to_string())
    }

    pub fn replace(
        &self,
        items: impl IntoIterator<Item = BindingVirtualListItem>,
    ) -> Result<bool, String> {
        self.inner
            .replace(items.into_iter().map(|item| (item.key, item.text)))
            .map_err(|error| error.to_string())
    }
}

impl fmt::Debug for BindingVirtualListModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingVirtualListModel")
            .field("len", &self.len())
            .finish()
    }
}
