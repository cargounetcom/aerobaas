# Master plan — all products (from 15 Oct 2026)

Owner: Aurora Ember Cyber Bio Lab. Rule for every product: **paid only — no payment, no service.** Daily details: `docs/PROGRESS.md`.

## 1. Products and domains

| Product | Domain(s) | Hosting | Status 15 Oct |
|---|---|---|---|
| SovereignBaaS | sovereignbaas.eu, www., license. | Railway (website, admin, licensing); product runs on the client's server | root ✅, license. ⏳ cert, www ❌ DNS |
| SovereignSQL | (sql.sovereignbaas.eu later) | client's server | installer ✅ CI, licence ❌ |
| AeroBaaS | aerobaas-production.up.railway.app | Railway | Phase 2a ✅, AI ⏳ |
| IDE ascodex | ascodex.com (landing), ide.ascodex.com (app) | Vercel / Railway | app ✅ HTTPS; sign-up form ❌; checkout ⏳ |
| ASCodex | ascodex.app, www.ascodex.app | Vercel (+ duplicate on Railway US) | ✅ loads; duplicate to decide |
| TemplateAI Studio | templateai.app | Vercel | ✅ loads; USD prices + "5 free" ❌ (decision: no free) |

## 2. Domain test — the same checklist for every domain

| # | Check | Pass when |
|---|---|---|
| 1 | DNS | A/CNAME point to the right host; no parking records |
| 2 | HTTPS | certificate valid, more than 14 days left |
| 3 | Redirects | http → https; www → main domain (or the reverse, one way only) |
| 4 | Page | HTTP 200, correct title, no console errors |
| 5 | Company | footer says "Aurora Ember Cyber Bio Lab", Ireland, contact e-mail |
| 6 | Legal | Terms, Privacy, Cookies pages exist and name the right company |
| 7 | Prices | same currency and amounts as in Stripe; no free tier |
| 8 | Checkout | test payment → licence key / access e-mail |
| 9 | Security headers | HSTS, X-Content-Type-Options, frame protection |
| 10 | Monitoring | uptime + certificate expiry alert configured |

Automation (week 2): one script `domain-check` runs checks 1–4, 9 for all domains every day and sends an alert; 5–8 are checked by hand once per release.

## 3. Product polish — "100 %" means

1. **One price list** — website = Stripe = licensing service.
2. **Full journey works:** visit → sign up → pay → licence/access → use → cancel (service stops after grace).
3. **E-mails:** welcome, payment receipt, licence key, renewal failed, licence expired.
4. **Docs:** quick start, install guide, FAQ, changelog.
5. **Legal:** Terms, Privacy, DPA (B2B), Cookies — correct company name.
6. **Quality:** CI green, no failing tests, backups + restore tested, monitoring on.
7. **Honest copy:** no feature claimed on the website unless a test proves it.

## 4. Timeline

| When | Product | Work |
|---|---|---|
| **16 Oct** | SovereignBaaS | DNS fix, paid licence merged, installer settings, PC self-test + template fixes, Stripe checkout → licence e-mail, SovereignSQL licence, restore + CI |
| **17 Oct** | TemplateAI | prices (owner confirms EUR), remove free generations, company name, checkout test, domain checklist |
| **18 Oct** | ASCodex + IDE ascodex | ASCodex: Railway duplicate decision, domain checklist, checkout test. IDE: sign-up form, checkout test, fix 8 old tests |
| **Week 2** (20–24 Oct) | all | `domain-check` script + daily alerts; legal pages check on every domain; SovereignBaaS: compliance pack (DPA, GDPR, security PDF), monitoring/alerts, comparison page |
| **Week 3–4** | all | e-mails (welcome, receipt, licence, expiry) per product; docs + quick starts; AeroBaaS Phase 2b (AI) |
| **Month 2–3** | SovereignBaaS | white-label kit, Studio (many servers in one admin), Discord/landing coupons, netcup affiliate link |
| **Month 2–3** | platform | Amplication/Prisma import, Sites tests, Next.js starter for Vercel |

## 5. Owner decisions still open

* SovereignBaaS prices: Starter / Pro / Studio per month (needed 16 Oct for Stripe).
* TemplateAI currency and prices (EUR €15 / €29 / €300 or the USD prices on the site).
* ASCodex: delete or keep the Railway copy `ascodex-web` (US region).
* aionflow: remove the crypto payout option (recommended).
