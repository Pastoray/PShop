//! Dependency-free single-page PDF invoice with an HMAC-SHA256 signature line.
//!
//! Deliberately tiny: the point of this service is the pipeline, not typography.
//! The signature key comes from AWS Secrets Manager, which gives the secret a
//! real, verifiable job in the system.

use hmac::{Hmac, KeyInit, Mac};
use pshop_common::events::OrderCommand;
use sha2::Sha256;

pub fn sign(key: &str, cmd: &OrderCommand) -> String {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(
        format!(
            "{}|{}|{}|{}|{}",
            cmd.order_id,
            cmd.user_id,
            cmd.sku,
            cmd.qty,
            cmd.total_cents()
        )
        .as_bytes(),
    );
    hex::encode(mac.finalize().into_bytes())
}

/// Deterministic per order, so a retry overwrites the same object (idempotent PUT).
pub fn object_key(cmd: &OrderCommand) -> String {
    format!(
        "invoices/{}/{}.pdf",
        cmd.accepted_at.format("%Y/%m"),
        cmd.order_id
    )
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

pub fn render(cmd: &OrderCommand, signature: &str) -> Vec<u8> {
    let total = cmd.total_cents();
    let lines = [
        format!("Order:     {}", cmd.order_id),
        format!("Customer:  {}", cmd.user_id),
        format!("Item:      {} x {}", cmd.sku, cmd.qty),
        format!(
            "Unit:      {}.{:02}",
            cmd.unit_price_cents / 100,
            cmd.unit_price_cents % 100
        ),
        format!("Total:     {}.{:02}", total / 100, total % 100),
        format!("Accepted:  {}", cmd.accepted_at.to_rfc3339()),
        format!("Signature: {}", &signature[..32.min(signature.len())]),
    ];

    let mut content = String::from("BT\n/F1 20 Tf\n72 760 Td\n(PShop Invoice) Tj\n/F1 11 Tf\n");
    for l in &lines {
        content.push_str(&format!("0 -22 Td\n({}) Tj\n", escape(l)));
    }
    content.push_str("ET\n");

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"
            .to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content),
    ];

    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, body));
    }
    let xref_at = out.len();
    out.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for off in offsets {
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_at
    ));
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn cmd() -> OrderCommand {
        OrderCommand {
            order_id: Uuid::now_v7(),
            user_id: "u-1".into(),
            sku: "SKU-0001".into(),
            qty: 2,
            unit_price_cents: 1999,
            stock_remaining: 10,
            stock_version: 1,
            idempotency_key: "k".into(),
            accepted_at: Utc::now(),
        }
    }

    #[test]
    fn pdf_is_structurally_valid() {
        let c = cmd();
        let pdf = render(&c, &sign("k", &c));
        let s = String::from_utf8(pdf).unwrap();
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.trim_end().ends_with("%%EOF"));
        // xref offset must point exactly at the "xref" keyword
        let start: usize = s
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(s[start..].starts_with("xref"));
    }

    #[test]
    fn signature_depends_on_key_and_content() {
        let c = cmd();
        assert_ne!(sign("a", &c), sign("b", &c));
        assert_eq!(sign("a", &c), sign("a", &c));
    }
}
