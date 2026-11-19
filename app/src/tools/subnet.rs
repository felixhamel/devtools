//! IP Subnet Calculator: details of an IPv4 or IPv6 CIDR block (with the
//! addresses AWS reserves in each VPC subnet), splitting it into smaller
//! subnets, and checking where an address or another block falls.
use std::fmt::Write as _;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use eframe::egui;
use ipnet::IpNet;
use super::{Category, Tool};
use crate::ui::{self, icon, theme};

/// The example: a typical VPC block…
const SAMPLE: &str = "10.0.0.0/16";
/// …an address to locate in it…
const SAMPLE_CHECK: &str = "10.0.37.12";
/// …and the prefix length it is split into.
const SAMPLE_SPLIT: u8 = 20;
/// Subnets listed at most (their total is always shown).
const MAX_LISTED: usize = 1024;
/// Above this many subnets, the map shows one bar with their count instead of a segment each.
const MAX_SEGMENTS: u128 = 256;
/// Split sizes offered: at most this many bits more than the block (65,536 subnets).
const MAX_SPLIT_BITS: u8 = 16;
/// Prefix lengths AWS accepts for an IPv4 VPC or subnet.
const AWS_SIZES: std::ops::RangeInclusive<u8> = 16..=28;
/// Addresses AWS reserves in every subnet: the first four and the last one.
const AWS_RESERVED: u128 = 5;

/// Parses `10.0.0.0/16`, `10.0.0.5 255.255.255.0` (or `/255.255.255.0`),
/// `2600:1f18::/56`, or a bare address (a /32 or /128 block).
fn parse(input: &str) -> Result<IpNet, String> {
    let input = input.trim();
    if let Ok(net) = input.parse::<IpNet>() { return Ok(net); }
    let invalid = || format!("“{input}” is not an IP address or CIDR block (e.g. 10.0.0.0/16)");
    let (addr, mask) = match input.split_once(|c: char| c == '/' || c.is_whitespace()) {
        Some((addr, mask)) => (addr, Some(mask.trim())),
        None => (input, None),
    };
    let addr: IpAddr = addr.parse().map_err(|_| invalid())?;
    let bits = if addr.is_ipv4() { 32 } else { 128 };
    let prefix = match mask {
        None => bits,
        // `10.0.0.0 16` or `10.0.0.0/ 16`: a prefix length after a space.
        Some(mask) if mask.parse::<u16>().is_ok_and(|n| n <= u16::from(bits)) => mask.parse().unwrap_or(bits),
        Some(mask) if mask.parse::<u16>().is_ok() => return Err(format!("The prefix length must be between 0 and {bits}")),
        Some(mask) => match (addr, mask.parse::<Ipv4Addr>()) {
            (IpAddr::V4(_), Ok(mask)) => ipnet::ipv4_mask_to_prefix(mask).map_err(|_| format!("{mask} is not a netmask: its 1 bits must all come first"))?,
            _ => return Err(invalid()),
        },
    };
    IpNet::new(addr, prefix).map_err(|_| invalid())
}

/// Bits in an address of the block's family: 32 or 128.
fn bits(net: &IpNet) -> u8 { if matches!(net, IpNet::V4(_)) { 32 } else { 128 } }

/// The address as a number, for offset arithmetic (IPv4 fits in the low 32 bits).
fn to_u128(addr: IpAddr) -> u128 {
    match addr { IpAddr::V4(a) => u32::from(a) as u128, IpAddr::V6(a) => u128::from(a) }
}

/// Number of addresses in a block, or `None` for 2^128 (all of IPv6).
fn size(net: &IpNet) -> Option<u128> { 1u128.checked_shl(u32::from(bits(net) - net.prefix_len())) }

/// How many subnets with prefix length `prefix` the block holds.
fn subnet_count(net: &IpNet, prefix: u8) -> u128 { 1u128 << (prefix - net.prefix_len()) }

/// 65536 → "65,536".
fn group(n: u128) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) { out.push(','); }
        out.push(c);
    }
    out
}

/// Address count for people: exact when short, as a power of two otherwise.
fn count_text(net: &IpNet) -> String {
    let host_bits = bits(net) - net.prefix_len();
    match size(net) {
        Some(n) if host_bits <= 48 => group(n),
        _ => format!("2^{host_bits} (≈ {:.2e})", 2f64.powi(i32::from(host_bits))),
    }
}

/// What an IPv4 address is used for: private, public, loopback…
fn kind_v4(a: Ipv4Addr) -> &'static str {
    let [first, second, ..] = a.octets();
    if a.is_private() { "Private (RFC 1918)" }
    // 100.64.0.0/10: first octet 100, second octet 64 to 127 (top bits 01).
    else if first == 100 && (second & 0xC0) == 64 { "Shared address space / carrier-grade NAT (RFC 6598)" }
    else if a.is_loopback() { "Loopback" }
    else if a.is_link_local() { "Link-local (e.g. 169.254.169.254, the instance metadata service)" }
    else if a.is_multicast() { "Multicast" }
    else if first == 0 { "\"This network\" (0.0.0.0/8)" }
    else if first >= 240 { "Reserved" }
    else { "Public" }
}

/// What an IPv6 address is used for, from its first 16 bits.
fn kind_v6(a: Ipv6Addr) -> &'static str {
    let first = a.segments()[0];
    if a.is_unspecified() { "Unspecified" }
    else if a.is_loopback() { "Loopback" }
    else if a.is_multicast() { "Multicast" }
    else if first & 0xFE00 == 0xFC00 { "Unique local (private, fc00::/7)" }
    else if first & 0xFFC0 == 0xFE80 { "Link-local (fe80::/10)" }
    else if first & 0xE000 == 0x2000 { "Global unicast (public)" }
    else { "Reserved" }
}

/// The block's details, as a `Key: value` report.
fn report(net: &IpNet) -> String {
    let mut lines = Vec::new();
    if net.addr() != net.network() { lines.push(format!("Address: {} (a host inside the block)", net.addr())); }
    lines.push(format!("Network: {}", net.trunc()));
    match net {
        IpNet::V4(v4) => {
            let (first, last) = (u32::from(v4.network()), u32::from(v4.broadcast()));
            let ip = |n: u32| Ipv4Addr::from(n).to_string();
            lines.push(format!("Netmask: {}", v4.netmask()));
            lines.push(format!("Wildcard: {}", v4.hostmask()));
            lines.push(format!("Range: {} – {}", ip(first), ip(last)));
            lines.push(format!("Broadcast: {}", ip(last)));
            lines.push(format!("Addresses: {}", count_text(net)));
            lines.push(match v4.prefix_len() {
                32 => "Usable hosts: 1 (a single address)".to_owned(),
                31 => "Usable hosts: 2 (point-to-point link, RFC 3021)".to_owned(),
                // The network and broadcast addresses are not hosts.
                _ => format!("Usable hosts: {} ({} – {})", group(u128::from(last - first - 1)), ip(first + 1), ip(last - 1)),
            });
            lines.push(String::new());
            if AWS_SIZES.contains(&v4.prefix_len()) {
                // AWS reserves the network address, +1 (VPC router), +2 (DNS),
                // +3 (future use) and the broadcast address.
                lines.push("AWS: valid VPC or subnet size (/16 to /28)".to_owned());
                lines.push(format!("AWS usable: {} ({} – {})", group(u128::from(last - first) + 1 - AWS_RESERVED), ip(first + 4), ip(last - 1)));
                lines.push(format!("AWS reserved: {}, {}, {}, {}, {}", ip(first), ip(first + 1), ip(first + 2), ip(first + 3), ip(last)));
            } else {
                let why = if v4.prefix_len() < 16 { "too large for a VPC or subnet (/16 at most)" } else { "too small for a VPC or subnet (/28 at least)" };
                lines.push(format!("AWS: {why}"));
            }
            lines.push(String::new());
            lines.push(format!("Type: {}", kind_v4(v4.network())));
            let class = match v4.network().octets()[0] { 0..=127 => "A", 128..=191 => "B", 192..=223 => "C", 224..=239 => "D (multicast)", _ => "E (reserved)" };
            lines.push(format!("Class: {class}"));
            let mask = v4.netmask().octets().map(|o| format!("{o:08b}")).join(".");
            lines.push(format!("Binary netmask: {mask}"));
            lines.push(format!("Integer range: {first} – {last}"));
        }
        IpNet::V6(v6) => {
            lines.push(format!("Range: {} – {}", v6.network(), v6.broadcast()));
            lines.push(format!("Addresses: {}", count_text(net)));
            if v6.prefix_len() <= 64 { lines.push(format!("/64 subnets: {}", group(subnet_count(net, 64)))); }
            lines.push(format!("Netmask: {}", v6.netmask()));
            lines.push(format!("Expanded: {}", v6.network().segments().map(|s| format!("{s:04x}")).join(":")));
            lines.push(String::new());
            lines.push(format!("Type: {}", kind_v6(v6.network())));
        }
    }
    lines.join("\n")
}

/// The first `limit` subnets of `net` with prefix `prefix`.
fn split(net: &IpNet, prefix: u8, limit: usize) -> Vec<IpNet> {
    net.trunc().subnets(prefix).map(|subnets| subnets.take(limit).collect()).unwrap_or_default()
}

/// Usable addresses in each subnet of size `prefix`, the AWS way for valid IPv4 sizes.
fn usable_per_subnet(net: &IpNet, prefix: u8) -> Option<u128> {
    let size = 1u128.checked_shl(u32::from(bits(net) - prefix))?;
    match net {
        IpNet::V4(_) if AWS_SIZES.contains(&prefix) => Some(size - AWS_RESERVED),
        IpNet::V4(_) if prefix <= 30 => Some(size - 2),
        _ => Some(size),
    }
}

/// Where a checked address or block lies relative to the block.
#[derive(Debug, PartialEq)]
enum Relation {
    Same,
    /// The other one is inside the block.
    Inside,
    /// The other one contains the whole block.
    Contains,
    /// No address in common.
    Disjoint,
}

/// Where `other` lies relative to `net`, both taken as whole blocks.
fn relation(net: &IpNet, other: &IpNet) -> Relation {
    let (a, b) = (net.trunc(), other.trunc());
    if a == b { Relation::Same }
    else if a.contains(&b) { Relation::Inside }
    else if b.contains(&a) { Relation::Contains }
    else { Relation::Disjoint }
}

/// Position of `subnet` among the block's subnets of its size: its offset in
/// the block divided by the subnet size.
fn index_of(net: &IpNet, subnet: &IpNet) -> u128 {
    let offset = to_u128(subnet.network()) - to_u128(net.network());
    offset >> (bits(net) - subnet.prefix_len())
}

/// Where the checked address or block lies.
struct Check {
    /// The badge text and color, or why the input is not an address.
    verdict: Result<(String, egui::Color32), String>,
    /// The parsed address or block.
    other: Option<IpNet>,
    /// With a split: the subnet that holds it.
    holder: Option<IpNet>,
}

/// The IP Subnet Calculator tool: a block, its details, an optional split and an address to check.
#[derive(Default)]
pub struct SubnetCalculator {
    input: String,
    /// An address or block to locate in the block.
    check: String,
    /// Prefix length of the subnets to list, if splitting.
    split: Option<u8>,
    /// The parsed block; `None` while the input is empty or invalid.
    net: Option<IpNet>,
    error: String,
    /// The block's details, as `Key: value` lines.
    report: String,
    /// The first [`MAX_LISTED`] subnets of the split.
    subnets: Vec<IpNet>,
}

impl SubnetCalculator {
    /// Parses the input again and recomputes the details and the subnets.
    fn update(&mut self) {
        self.error.clear();
        self.report.clear();
        self.subnets.clear();
        self.net = None;
        if self.input.trim().is_empty() { return; }
        match parse(&self.input) {
            Ok(net) => {
                self.report = report(&net);
                // A split size only makes sense for smaller subnets of the same block.
                // Same limits as the "Split into" choices, so the subnet count always fits.
                self.split = self.split.filter(|&p| p > net.prefix_len() && p <= bits(&net) && p - net.prefix_len() <= MAX_SPLIT_BITS);
                if let Some(prefix) = self.split { self.subnets = split(&net, prefix, MAX_LISTED); }
                self.net = Some(net);
            }
            Err(e) => self.error = e,
        }
    }

    /// The checked address or block: where it lies, as a colored badge, and
    /// the subnet that holds it (with a split).
    fn check_result(&self, net: &IpNet) -> Option<Check> {
        let text = self.check.trim();
        if text.is_empty() { return None; }
        let other = match parse(text) { Ok(other) => other, Err(e) => return Some(Check { verdict: Err(e), other: None, holder: None }) };
        let single = other.prefix_len() == bits(&other);
        // With a split, the subnet holding `other` is its network truncated to the
        // split's prefix, if it fits in one subnet and lies in the block.
        let holder = self.split.filter(|&p| p <= other.prefix_len() && net.trunc().contains(&other))
            .and_then(|p| IpNet::new(other.network(), p).ok()).map(|s| s.trunc());
        let result = match (relation(net, &other), single) {
            (Relation::Inside | Relation::Same, true) => (match holder {
                Some(s) => format!("Inside · subnet #{} {s}", index_of(net, &s) + 1),
                None => "Inside this block".to_owned(),
            }, theme::ACCENT),
            (Relation::Disjoint, true) => ("Outside this block".to_owned(), theme::MUTED),
            (Relation::Same, false) => ("Same block: they overlap".to_owned(), theme::WARNING),
            (Relation::Inside, false) => (match holder {
                Some(s) => format!("Inside (overlaps) · subnet #{} {s}", index_of(net, &s) + 1),
                None => "Inside this block (they overlap)".to_owned(),
            }, theme::ACCENT),
            (Relation::Contains, _) => ("Contains this whole block: they overlap".to_owned(), theme::WARNING),
            (Relation::Disjoint, false) => ("No overlap: OK for VPC peering or routing".to_owned(), theme::ACCENT),
        };
        Some(Check { verdict: Ok(result), other: Some(other), holder })
    }

    /// The "Split into" choices: "/20 · 16 subnets · 4,091 usable each".
    fn split_label(net: &IpNet, prefix: u8) -> String {
        let count = subnet_count(net, prefix);
        let usable = usable_per_subnet(net, prefix).map_or_else(|| "2^128".into(), group);
        let each = if matches!(net, IpNet::V4(_)) && AWS_SIZES.contains(&prefix) { "usable each on AWS" } else { "usable each" };
        format!("/{prefix} · {} {} · {usable} {each}", group(count), if count == 1 { "subnet" } else { "subnets" })
    }

    /// A bar standing for the block's address space: the subnets as segments
    /// (hover one to see it) and the checked address or block highlighted.
    fn map(&self, ui: &mut egui::Ui, net: &IpNet, checked: Option<IpNet>, holder: Option<IpNet>) {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, theme::SEARCH_BG);
        // Where an address falls along the bar, from 0 to 1.
        let fraction = |addr: IpAddr| {
            let offset = to_u128(addr).saturating_sub(to_u128(net.network())) as f64;
            let total = size(net).map_or(2f64.powi(128), |n| n as f64);
            (offset / total) as f32
        };
        let count = self.split.map_or(1, |p| subnet_count(net, p));
        let inner = rect.shrink(3.0);
        let held = holder.map(|h| index_of(net, &h) as usize);
        if count <= MAX_SEGMENTS {
            for i in 0..count as usize {
                let x0 = inner.left() + inner.width() * i as f32 / count as f32;
                let x1 = inner.left() + inner.width() * (i + 1) as f32 / count as f32;
                // A 1-point gap between segments.
                let segment = egui::Rect::from_x_y_ranges(x0..=(x1 - if count > 1 { 1.0 } else { 0.0 }), inner.y_range());
                let hovered = response.hover_pos().is_some_and(|p| segment.contains(p));
                let fill = if hovered { theme::ACCENT_STRONG } else if held == Some(i) { theme::HOT.gamma_multiply(0.45) } else if i % 2 == 0 { theme::ACCENT_DARK.gamma_multiply(0.75) } else { theme::ACCENT_DARK.gamma_multiply(0.5) };
                painter.rect_filled(segment, 3.0, fill);
                if hovered {
                    let label = self.subnets.get(i).map_or_else(|| net.trunc().to_string(), |s| format!("#{} {s}", i + 1));
                    response.clone().on_hover_text_at_pointer(label);
                }
            }
        } else {
            painter.rect_filled(inner, 3.0, theme::ACCENT_DARK.gamma_multiply(0.6));
            painter.text(inner.center(), egui::Align2::CENTER_CENTER, format!("{} subnets", group(count)), egui::FontId::proportional(12.0), theme::TEXT_STRONG);
        }
        // The checked address or block, at least 3 points wide so a single address shows.
        if let Some(other) = checked.filter(|o| net.trunc().contains(&o.trunc())) {
            let (start, end) = (fraction(other.network()), fraction(other.broadcast()));
            let x0 = inner.left() + inner.width() * start;
            let x1 = (inner.left() + inner.width() * end).max(x0 + 3.0);
            let mark = egui::Rect::from_x_y_ranges(x0..=x1, rect.y_range());
            painter.rect_filled(mark, 2.0, theme::HOT);
        }
        let font = egui::FontId::monospace(11.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(net.network().to_string()).font(font.clone()).color(theme::SUBTLE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(net.broadcast().to_string()).font(font).color(theme::SUBTLE));
            });
        });
    }

    /// A reference of the IPv4 sizes AWS accepts (/16 to /28). The block's own
    /// size is highlighted; clicking a smaller one splits the block into it.
    fn sizes(&mut self, ui: &mut egui::Ui, net: &IpNet) {
        ui::toolbar(ui, "AWS sizes:", |_| {});
        let mut chosen = None;
        ui::preview_frame(ui, |ui| {
            egui::Grid::new("aws-sizes").num_columns(4).spacing([18.0, 4.0]).striped(true).show(ui, |ui| {
                for header in ["Prefix", "Netmask", "Addresses", "AWS usable"] { ui.label(egui::RichText::new(header).small().color(theme::SUBTLE)); }
                ui.end_row();
                for prefix in AWS_SIZES {
                    let addresses = 1u128 << (32 - prefix);
                    let mask = Ipv4Addr::from(u32::MAX << (32 - prefix));
                    let color = if prefix == net.prefix_len() { theme::ACCENT } else if Some(prefix) == self.split { theme::HOT } else { theme::TEXT_STRONG };
                    let label = egui::RichText::new(format!("/{prefix}")).monospace().color(color);
                    if prefix > net.prefix_len() {
                        let link = ui.add(egui::Button::new(label).frame(false)).on_hover_text(format!("Split {} into /{prefix}", net.trunc()));
                        if link.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { chosen = Some(prefix); }
                    } else {
                        ui.label(label);
                    }
                    ui.label(egui::RichText::new(mask.to_string()).monospace().color(theme::MUTED));
                    ui.label(egui::RichText::new(group(addresses)).monospace().color(theme::MUTED));
                    ui.label(egui::RichText::new(group(addresses - AWS_RESERVED)).monospace().color(color));
                    ui.end_row();
                }
            });
        });
        if chosen.is_some() {
            self.split = chosen;
            self.update();
        }
    }

    /// The listed subnets: number, CIDR, range and a copy button; the one
    /// holding the checked address is highlighted.
    fn subnet_table(&self, ui: &mut egui::Ui, net: &IpNet, holder: Option<IpNet>) {
        let total = self.split.map_or(0, |p| subnet_count(net, p));
        ui::preview_frame(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("subnet-table").show(ui, |ui| {
                egui::Grid::new("subnets").num_columns(4).spacing([14.0, 5.0]).striped(true).show(ui, |ui| {
                    for header in ["#", "Subnet", "Range", ""] { ui.label(egui::RichText::new(header).small().color(theme::SUBTLE)); }
                    ui.end_row();
                    for (i, subnet) in self.subnets.iter().enumerate() {
                        let held = holder == Some(*subnet);
                        let color = if held { theme::HOT } else { theme::TEXT_STRONG };
                        ui.label(egui::RichText::new((i + 1).to_string()).monospace().color(theme::SUBTLE));
                        ui.label(egui::RichText::new(subnet.to_string()).monospace().color(color));
                        ui.label(egui::RichText::new(format!("{} – {}", subnet.network(), subnet.broadcast())).monospace().color(theme::MUTED));
                        ui::copy_icon(ui, &subnet.to_string(), ("subnet", i));
                        ui.end_row();
                    }
                });
            });
            if total > self.subnets.len() as u128 {
                ui.label(egui::RichText::new(format!("… and {} more", group(total - self.subnets.len() as u128))).color(theme::SUBTLE));
            }
        });
    }
}

impl Tool for SubnetCalculator {
    fn name(&self) -> &'static str { "IP Subnet Calculator" }
    fn icon(&self) -> &'static str { icon::NETWORK }
    fn category(&self) -> Category { Category::Network }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui::description(ui, "Details of an IPv4 or IPv6 CIDR block, with the addresses AWS reserves in each subnet. Split it into subnets, or check where an address or another block falls.");
        ui.add_space(10.0);
        ui::split(ui, |left, right| {
            let mut changed = false;
            ui::toolbar(left, "CIDR block:", |ui| {
                changed |= ui::paste_button(ui, &mut self.input, &mut self.error);
                if ui::sample_button(ui) { self.load_sample(); }
                if ui::clear_button(ui) { *self = Self::default(); }
            });
            let field = ui::editor_frame().show(left, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.input).frame(false).font(egui::FontId::monospace(18.0))
                    .desired_width(f32::INFINITY).hint_text("10.0.0.0/16"))
            }).inner;
            changed |= field.changed();
            if changed { self.update(); }
            left.add_space(6.0);
            ui::error_label(left, &self.error);
            let Some(net) = self.net else {
                left.label(egui::RichText::new("Also accepted: 192.168.1.10 255.255.255.0, a single address, or IPv6 such as 2600:1f18:abcd::/56.").color(theme::SUBTLE));
                ui::preview_frame(right, |ui| ui::empty_state(ui, icon::NETWORK, "Enter a CIDR block on the left."));
                return;
            };
            let check = self.check_result(&net);
            egui::Grid::new("subnet-options").num_columns(2).spacing([12.0, 10.0]).show(left, |ui| {
                ui.label(egui::RichText::new("Split into").color(theme::MUTED));
                let selected = self.split.map_or_else(|| "Don't split".to_owned(), |p| Self::split_label(&net, p));
                let mut split = self.split;
                egui::ComboBox::from_id_salt("subnet-split").selected_text(selected).width(ui.available_width()).show_ui(ui, |ui| {
                    ui.selectable_value(&mut split, None, "Don't split");
                    let last = bits(&net).min(net.prefix_len().saturating_add(MAX_SPLIT_BITS));
                    for prefix in net.prefix_len() + 1..=last { ui.selectable_value(&mut split, Some(prefix), Self::split_label(&net, prefix)); }
                });
                if split != self.split { self.split = split; self.update(); }
                ui.end_row();
                ui.label(egui::RichText::new("Check").color(theme::MUTED)).on_hover_text("An address or a block: where does it fall?");
                ui.add(egui::TextEdit::singleline(&mut self.check).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY)
                    .hint_text("An IP or CIDR, e.g. 10.0.37.12 or 10.1.0.0/16"));
                ui.end_row();
            });
            left.add_space(8.0);
            match check.as_ref().map(|c| &c.verdict) {
                Some(Ok((text, color))) => ui::badge(left, icon::CROSSHAIR, text, *color),
                Some(Err(e)) => { left.label(egui::RichText::new(e).color(theme::ERROR_TEXT)); }
                None => {}
            }
            left.add_space(12.0);
            let holder = check.as_ref().and_then(|c| c.holder);
            self.map(left, &net, check.as_ref().and_then(|c| c.other), holder);
            if let IpNet::V4(_) = net {
                left.add_space(14.0);
                self.sizes(left, &net);
            }

            let details_height = if self.split.is_some() { (right.available_height() * 0.55).max(160.0) } else { right.available_height() };
            right.allocate_ui(egui::vec2(right.available_width(), details_height), |ui| {
                ui::toolbar(ui, "Details:", |ui| ui::copy_button(ui, &self.report));
                ui::properties(ui, "subnet-details", &self.report);
            });
            if let Some(prefix) = self.split {
                right.add_space(8.0);
                let list = self.subnets.iter().fold(String::new(), |mut list, s| { let _ = writeln!(list, "{s}"); list });
                ui::toolbar(right, &format!("Subnets /{prefix} ({}):", group(subnet_count(&net, prefix))), |ui| ui::copy_button(ui, &list));
                self.subnet_table(right, &net, holder);
            }
        });
    }

    fn load_sample(&mut self) {
        self.input = SAMPLE.into();
        self.check = SAMPLE_CHECK.into();
        self.split = Some(SAMPLE_SPLIT);
        self.update();
    }

    fn accepts_input(&self) -> bool { true }

    fn set_input(&mut self, text: &str) {
        self.input = text.trim().to_owned();
        self.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_usual_notations() {
        assert_eq!(parse("10.0.0.0/16").unwrap().to_string(), "10.0.0.0/16");
        assert_eq!(parse(" 192.168.1.10 255.255.255.0 ").unwrap().to_string(), "192.168.1.10/24");
        assert_eq!(parse("192.168.1.10/255.255.255.192").unwrap().to_string(), "192.168.1.10/26");
        assert_eq!(parse("8.8.8.8").unwrap().to_string(), "8.8.8.8/32");
        assert_eq!(parse("2600:1f18:abcd::/56").unwrap().prefix_len(), 56);
        assert!(parse("10.0.0.0/33").unwrap_err().contains("between 0 and 32"));
        assert_eq!(parse("10.0.0.0 16").unwrap().to_string(), "10.0.0.0/16");
        assert_eq!(parse("10.0.0.0/ 16").unwrap().to_string(), "10.0.0.0/16");
        assert!(parse("10.0.0.0 255.0.255.0").unwrap_err().contains("not a netmask"));
        assert!(parse("hello").is_err());
    }

    #[test]
    fn reports_aws_usable_and_reserved_addresses() {
        let r = report(&parse("10.0.0.0/24").unwrap());
        assert!(r.contains("Netmask: 255.255.255.0\nWildcard: 0.0.0.255\nRange: 10.0.0.0 – 10.0.0.255"), "{r}");
        assert!(r.contains("Usable hosts: 254 (10.0.0.1 – 10.0.0.254)"), "{r}");
        assert!(r.contains("AWS usable: 251 (10.0.0.4 – 10.0.0.254)"), "{r}");
        assert!(r.contains("AWS reserved: 10.0.0.0, 10.0.0.1, 10.0.0.2, 10.0.0.3, 10.0.0.255"), "{r}");
        assert!(r.contains("Type: Private (RFC 1918)\nClass: A"), "{r}");
        assert!(report(&parse("10.0.0.0/8").unwrap()).contains("AWS: too large"));
        assert!(report(&parse("10.0.0.0/29").unwrap()).contains("AWS: too small"));
        assert!(report(&parse("10.0.5.7/16").unwrap()).starts_with("Address: 10.0.5.7"));
        assert!(report(&parse("100.64.1.0/24").unwrap()).contains("carrier-grade NAT"));
        assert!(report(&parse("2600:1f18:abcd::/56").unwrap()).contains("/64 subnets: 256"));
        assert!(report(&parse("::/0").unwrap()).contains("Addresses: 2^128"));
    }

    #[test]
    fn splits_and_locates() {
        let net = parse("10.0.0.0/16").unwrap();
        let subnets = split(&net, 20, MAX_LISTED);
        assert_eq!(subnets.len(), 16);
        assert_eq!((subnets[0].to_string(), subnets[15].to_string()), ("10.0.0.0/20".into(), "10.0.240.0/20".into()));
        assert_eq!(usable_per_subnet(&net, 20), Some(4091));
        assert_eq!(usable_per_subnet(&net, 30), Some(2));
        assert_eq!(relation(&net, &parse("10.0.37.12").unwrap()), Relation::Inside);
        assert_eq!(relation(&net, &parse("10.1.0.0/16").unwrap()), Relation::Disjoint);
        assert_eq!(relation(&net, &parse("10.0.0.0/8").unwrap()), Relation::Contains);
        assert_eq!(index_of(&net, &parse("10.0.32.0/20").unwrap()), 2);
        assert_eq!(group(65536), "65,536");
    }

    #[test]
    fn drops_a_split_that_no_longer_fits() {
        // A /128 split chosen on a /112 must not survive the block becoming /0 (1 << 128 overflows).
        let mut calc = SubnetCalculator { input: "2001:db8::/112".into(), split: Some(128), ..Default::default() };
        calc.update();
        assert_eq!(calc.split, Some(128));
        calc.input = "2001:db8::/0".into();
        calc.update();
        assert_eq!(calc.split, None);
    }
}
