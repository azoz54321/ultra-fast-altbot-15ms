# Ultra-Fast Altcoin Bot (≤15ms tick→BUY+LIMIT p95)

هدف المشروع: إرسال BUY ثم LIMIT (TP +10%) خلال ≤15ms (p95) من لحظة وصول صفقة (trade tick) على جانب التطبيق، مع شاردات، وبدون تخصيصات على المسار الحار. يبدأ في وضع SHADOW (اختبار بدون أوامر حقيقية).

## المراحل
- Phase 0: Skeleton + Shadow Mode.
- Phase 1: Hot Path (SBE ingest → decode FFI → ret_60s ≥5% → إشعال trigger).
- Phase 2: Integer sizing + tick/step rounding + rings 60s/15m/1h خارج المسار الحار.
- Phase 3: Alt Market Gate (AHI).
- Phase 4: Risk (Slots A/B، freeze، bans).
- Phase 5: Ops Hardening (reconnect، token-bucket، singleflight، filter refresh).
- Phase 6: Benchmark CI (p50/p95/p99 histograms، إسقاط التريجر فوق الميزانية).
- Phase 7: LIVE enable (يتطلب Secrets وبيئة محمية).

## التشغيل
```bash
cargo build --release
cargo run --release
```

التهيئة مبدئيًا في `src/config/mod.rs`، مع هدف p95 = 15ms ووضع SHADOW مفعّل.

## CI
- Build & Test: يبني ويشغل الاختبارات (فارغة الآن).
- Latency Bench (يدويًا): يشغل بنشمارك ظل بسيط ويرفع لوج كـ Artifact.

## ملاحظات
- هذا سكافولد أولي قابل للتوسعة. سنضيف وحدات: data_feed (SBE) و execution و alt_gate و rings تباعًا عبر PRات.
