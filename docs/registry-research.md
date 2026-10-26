# Registry research: unregistered names, journal runs, reporter editions

Researched 2026-10-25 for the registry's year checks (`Options::year_runs`) and for authored rows the registry
lacks. Nothing here is in the registry yet; rows go in from this record, each fact with its source.

## Method

- Names come from the citations the engine read in a 1,424-text sample of A2AJ decisions and journal articles,
  ranked by how often each is cited (`docs/research/*.json` keep each item's examples).
- Each fact was found on a page fetched raw (not through a summarizer), and then re-checked: every cited page
  was fetched again and the quote searched for in its tag-stripped text. Only facts that passed are listed as
  verified below. A library's holdings range is marked as such where the researcher said so; it is not the
  series' run.
- Blocked to raw fetches: the Cardiff Index to Legal Abbreviations (every page), HeinOnline, Villanova,
  Berkeley, several university catalogues, Wikipedia (rate limits) and, intermittently, Érudit. A fact only
  those sources hold is listed as not established.

## What the research changes

- **A decision's year can precede its report's series.** Registry examples cite "(1973) 1 OR (2d)" though OR
  (2d) began in 1974, and "(1968) 1 DLR (3d)" though DLR (3d) began in 1969: decisions are reported after they
  are decided. A citation's decision year rules out a series only after the series ended, or before it began
  where the year is the volume ("[1932] AC 562").
- **Journal volumes span years, and years hold several volumes.** Osgoode Hall LJ vol. 2 ran April 1960 to
  April 1963; Harvard, Queen's, Cornell, Georgetown and Iowa volumes span two years; McGill (1967), Dalhousie
  (1984) and the Modern LR (1938, 1939) printed two volumes in a year. Writers cite either end of a span
  ("(1983) 7 Dal LJ", vol. 7 no. 1 being 1982), and an anchor names whichever issue's year its page listed. So a
  volume's year is bounded only by its neighbours: no earlier than an anchored volume before it, no later than an
  anchored volume after it (that anchor's span where its page shows one). A volume's own anchor bounds nothing.
- **The English Reports' dates are the reprint's.** 1900-1932 is when the reprint appeared; its cases run
  1220-1865. ER takes no run.
- **One abbreviation, several series by period.** Quebec's C.S. is the Rapports judiciaires officiels
  (1892-1966) and then the Recueils de jurisprudence (1967-1985); C.A. 1970-1985 (earlier Court of Appeal
  reports, 1921-75, also used R.J.Q.); AC before 1891 is the 1875-1890 Appeal Cases; QB is LR QB 1865-1875, QBD
  1875-1890, QB 1891-1900, KB 1901-1952 and QB from 1953; Lloyd's List Law Reports (Ll L Rep) 1919-1950, then
  Lloyd's Law Reports. Such names need a row per period.
- **Registry rows the research contradicts.** "CLR" examples (51 CLR in 1934, 115 CLR in 1966) are the
  Australian Commonwealth Law Reports, not the Canadian Construction Law Reports; "(1911) WLR" is the Western
  Law Reporter, not the Weekly Law Reports; "rdmcgill" is the McGill Law Journal's French title (since 1977);
  the OSC Bulletin, the Gazette officielle du Québec, Quebec's Q.B. reports and R.S.Q. are filed as journals.
- **Corrections to registry years.** Cahiers de droit began 1954 (not 1959); Cornell Law Quarterly became the
  Cornell Law Review with vol. 53 in 1967 (not 1966); NYU's 1925 is its second volume (founded 1924); UNB LJ began
  1947 (not 1946); Upper Canada Q.B. New Series began 1842 (not 1844); the Law Times ran 1859-[1948].
- **Corrections to reporters-db years** (`registry/edition-years.json`, applied over the synced rows). reporters-db
  ends F. Supp. in 1988 and starts F. Supp. 2d in 1988; the first series ran to 1998 ([Wikipedia](https://en.wikipedia.org/wiki/Federal_Supplement):
  "Published: 1933–1998") and the second began in 1998 ([CiNii](https://ci.nii.ac.jp/ncid/AA11282919): "Vol. 1
  (1998)-v. 999 ([2014])"), so "728 F. Supp. 438 (E.D. Tex. 1989)" is no longer ruled out.
- **Not publications** (junk the generic grammar reads): "L'article", "No.", "Conclusion", "Background", and
  statute revisions read as reports ("1 R.S.C. 1985", "14 R.S.B.C. 1960").
- **Dotted neutral citations** ("2006 B.C.S.C. 503", "2001 C.A.F. 254") are courts the registry has; the
  grammar now reads the periods. First applications (Lexum table, via the NZ Law Style Guide): BCCA January
  1999, BCSC and SCC January 2000, FCA February 2001, CHRT January 2003, ONCA January 2007.

## Names the registry does not know

### R.S.C. — not a publication: 'N R.S.C. YEAR' (1 R.S.C. 1985, 10 R.S.C. 1970) is chapter N of the Revised Statutes of Canada of that year, a statute series cited by chapter, not a report. The lone '(1904), 35 R.S.C. 14' has a year before the volume, the report pattern; it is probably a mis-parse of another series but no fetched page says so.

Judgment from the example pattern only; no page fetched for it.

### larticle — not a publication: French words 'L'article N' (section N) after a footnote number; a pinpoint, not a publication.

### no — not a publication: 'No.' is an issue designator in '(YEAR), VOL No. N'; the journal title that should precede it is missing from the examples.

Cannot be resolved without the source documents.

### C.S. — Rapports judiciaires officiels de Québec: Cour supérieure (1892-1966); Recueils de jurisprudence du Québec: Cour supérieure (1967-1985)

Examples: [1959] and [1963] fall in the 1892-1966 series; [1974], [1976], [1977] fall in the 1967-1985 series. '5 C's 4' is not this series (not a publication). The NZ guide spells 'judiciaries' (sic) as quoted.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1892 (Rapports judiciaires officiels de Québec: Cour supérieure) | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | CS Rapports judiciaries officiels de Québec: Cour supérieure 1892–1966 |
| end | 1966 (first series) | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | CS Rapports judiciaries officiels de Québec: Cour supérieure 1892–1966 |
| start | 1967 (Recueils de jurisprudence du Québec: Cour supérieure) | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | CS Recueils de jurisprudence du Québec: Cour supérieure 1967–1985 |
| end | 1985 (second series) | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | CS Recueils de jurisprudence du Québec: Cour supérieure 1967–1985 |
| abbreviation | C.S. (with periods) | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | C.S. Recueils de jurisprudence du Québec. Cour supérieure |

Not established: numbering by year, cited [YEAR] C.S. page.

### B.C.C.A.A.A. — British Columbia Collective Agreement Arbitration Awards (probable expansion)

Related page says: 'Electronic sources (full text) 1974 - Quicklaw (search for Source: BCAR or British Columbia Collective Agreement Arbitration Awards'. That names a Quicklaw source (coverage 1974) but never writes B.C.C.A.A.A., so the equality of the abbreviation with that source and the 1974 start for this identifier stay unverified. Numbered awards ('No. 241'), per-year numbering is not stated.

Not established: name British Columbia Collective Agreement Arbitration Awards; start 1974.

### H.L.R. — Harvard Law Review

Laval's list gives H.L.R. = Harvard law review (the only expansion found). The page at harvardlawreview.org shows only a copyright range '1887-2026', not a statement of first publication, so start is left unverified. The examples (vols 16-21) are consistent with it but that is not proof.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Harvard Law Review | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | H.L.R. Harvard law review |

Not established: numbering continuous volumes; start 1887.

### C.A. — Recueils de jurisprudence du Québec: Cour d'appel

All examples ([1971]-[1976]) fall within 1970-1985. A separate catalogue record (https://sclqld.org.au/catalogue/records/19894, titled 'Recueils de jurisprudence du Quebec Cour d'Appel.') says 'Publication details: Montreal: Editeur officiel du Quebec, 1921-75.' and lists earlier Banc de la Reine/Court of Appeal reports among its added titles, so it describes a longer bibliographic run; it differs from the 1970 start. The earlier verified.json treated 1921-1975 as contradicting 1970-1985; I read it as a different bibliographic span (includes the BR series), not as a contradiction for the series the examples cite.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1970 | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | CA Recueils de jurisprudence du Québec: Cour d'appel 1970–1985 |
| end | 1985 | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | CA Recueils de jurisprudence du Québec: Cour d'appel 1970–1985 |
| abbreviation | C.A. (with periods) | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | C.A. Recueils de jurisprudence du Québec. Cour d'appel |

Not established: numbering by year, cited [YEAR] C.A. page.

### BCSC — Supreme Court of British Columbia (neutral citation)

Examples 2000-2007 all fall on/after the January 2000 adoption date. Page also says 'Note that some earlier decisions have been given what look like neutral citations by the Canadian Legal Information Institute (CanLII). These are not official'.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | January 2000 | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | Court of Appeal BCCA January 1999 Supreme Court BCSC January 2000 Provincial Court BCPC February 1999 |

Not established: numbering ordinal within year: YEAR BCSC N.

### Can. L.R.B.R. — Canadian Labour Relations Board Reports

The 1974-1982 original series (and a New Series from 1983) comes only from Cardiff search summaries; Cardiff returned 403 on every raw fetch, so no dates are claimed. Examples are [1975]-[1982].

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Canadian Labour Relations Board Reports | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | Can.L.R.B.R. Canadian Labour Relations Board Reports |
| abbreviation | C.L.R.B.R / Can.L.R.B.R. | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | C.L.R.B.R Canadian Labour Relations Board Reports |

Not established: end 1982; numbering volumes within each year: [YEAR] N Can LRBR page; start 1974.

### Regulation — Regulation

Policy magazine started by the American Enterprise Institute (Wikipedia: 'was started as a bimonthly magazine'; 'acquired by Cato in 1989'). Cato's own pages returned 403. Examples (1979) vol 3 etc. are consistent with year-based volumes but numbering is not stated by a fetched page. Cited by its full title, no abbreviation found.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1977 | <https://en.wikipedia.org/wiki/Regulation_(magazine)> | First issue July/August 1977 |

Not established: numbering volume per year from 1977.

### RLR — unresolved

Unresolved. Not in Laval's abbreviation list (only R.L. Revue légale and R.L.C./R.L.N.S. found). Several searches for [1995] RLR found nothing. Earlier files' guess (Refugee Law Reports) has no fetched source.

### BCCA — Court of Appeal for British Columbia (neutral citation)

Examples 1999-2004 are on/after adoption.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | January 1999 | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | Court of Appeal BCCA January 1999 Supreme Court BCSC January 2000 Provincial Court BCPC February 1999 |

Not established: numbering ordinal within year: YEAR BCCA N.

### SCC — Supreme Court of Canada (neutral citation)

Neutral citations from 2000 onward ('2000 S.C.C. 15', '[2008] SCC 1'). '[1978] 2 S. C. C. 1299' precedes 2000 so is not a neutral citation; likely a mis-read of the Supreme Court Reports (S.C.R.) but no fetched page states that, so left unresolved.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | January 2000 | <https://lawfoundation.org.nz/style-guide2019/appendix-3.html> | Supreme Court of Canada SCC, CSC January 2000 |

Not established: numbering ordinal within year: YEAR SCC N.

### R.S.S. — not a publication: 'N R.S.S. YEAR' (1 R.S.S. 1965, 8 R.S.S. 1967) is chapter N of the Revised Statutes of Saskatchewan of that year: a statute series cited by chapter, not a report.

Judgment from the example pattern; no page fetched.

### I.L.M. — International Legal Materials

Still publishing (now Cambridge University Press). The quote proves volume 1 began in 1962 but not that volumes are continuous rather than yearly, hence numbering unverified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | International Legal Materials | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | I.L.M. International Legal Materials |
| start | 1962 | <https://www.asil.org/resources/international-legal-materials> | Published on a bimonthly basis since 1962, International Legal Materials reproduces primary international legal documents |
| start | Vol. 1, no. 1 Aug. 1962 | <https://www.cca.qc.ca/en/search/details/library/publication/49234981> | Washington, D.C. : American Society of International Law, 1962- |
| end |  | <https://www.asil.org/resources/international-legal-materials> | Published on a bimonthly basis since 1962 |

Not established: numbering continuous volumes (Vol. 1 = 1962), issues within.

### Can. Y.B. Int'l L. — Canadian Yearbook of International Law

Page also says the volumes after 'As of 2015 (volume 52), The Canadian Yearbook of International Law is published by Cambridge' - still running. Examples (2000) 38 and (2002) 40 match one volume per year.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Canadian Yearbook of International Law = Annuaire canadien de droit international | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | Can.Y.B.Int.L Canadian Yearbook of International Law = Annuaire canadien de droit international |
| abbreviation | Can. Y.B. Int'l L | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | Can. Y.B. Int'l L Canadian Yearbook of International Law=Annuaire canadien de droit international |
| start | 1963 | <https://www.ubcpress.ca/canadian-yearbook-of-international-law> | the first volume of which was published in 1963 |
| numbering | continuous annual volumes, one per year (Vol. 1 = 1963; Vol. 50 = 2012) | <https://www.ubcpress.ca/canadian-yearbook-of-international-law> | The Canadian Yearbook of International Law, Vol. 50, 2012 |

Not established: end.

### P.L. — Public Law

Examples '[1999] P.L. 682', '[2000] P.L. 61', '1966 P.L. 89' fit this journal. '31 P.L. 100', '43 P.L. 235', '5 P.L. 48' have volume numbers and do not match a year-cited journal; they are probably another title (Laval's list also shows 'P.L.R. Patent Law Review') but this is not established. Wikipedia is secondary.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Public Law | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | P.L. Public Law |
| start | 1956 | <https://en.wikipedia.org/wiki/Public_Law_(journal)> | The journal was established in 1956 |
| end |  | <https://en.wikipedia.org/wiki/Public_Law_(journal)> | History 1956-present |

Not established: numbering cited by year [YEAR] P.L. page (journal).

### O.T.C. — unresolved

A fetched vLex page for an Ontario Superior Court of Justice judgment shows 'Citation [2002] O.T.C. 901 (SC)' and 'MLB headnote and full text' (a Maritime Law Book headnote), which places it as an Ontario trial-level reporter, but no fetched page expands the abbreviation or gives its dates. Not in Laval's list.

Not established: name Ontario Trial Cases (expansion from memory, not proven).

### L.T. — Law Times Reports

The same record: 'Vol. 1-17 issued as 'New series'; v. 1-4 also numbered v. 35-38, continuing the numbering of the old series.' The end year is in brackets in the catalogue (inferred by the cataloguer), and differs from the 1947 in earlier files (Cardiff summary, unfetched); I report the fetched 1948. Examples (1888 vol 57, 1922 vol 127) fall within 1859-1948.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Law Times Reports | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | L.T. Law Times Reports |
| abbreviation | LT. (Cited) | <https://www.courthouselibrary.ca/catalogue-item?cid=11840> | Cited: LT. |
| start | 1859 | <https://www.courthouselibrary.ca/catalogue-item?cid=11840> | Publication Date Range: 1859-[1948] |
| end | 1948 | <https://www.courthouselibrary.ca/catalogue-item?cid=11840> | Publication Date Range: 1859-[1948] |
| numbering | continuous volumes, New Series v.1 from Nov. 1859 | <https://www.courthouselibrary.ca/catalogue-item?cid=11840> | Beginning in Nov. 1859 the Law times reports were issued as a separate publication and numbered beginning with new ser., v. 1. |

### R.R.A. — Recueil en responsabilité et assurance

Cardiff (403) is the only source seen for a 1986 start, via a search summary, so unverified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Recueil en responsabilité et assurance | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | R.R.A. Recueil en responsabilité et assurance |

Not established: numbering by year, cited [YEAR] R.R.A. page; start 1986.

### T.T.R. — Trade and Tariff Reports

Laval's list gives the call number KE 6088.5 C213r (a Canadian-law class). No dates found. Earlier files' guesses (Trinidad, Territories Law Reports) are not supported.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Trade and Tariff Reports | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | T.T.R. Trade and Tariff Reports |

Not established: end; numbering continuous volumes (examples vol 1-3); start.

### R.T.R. — Road Traffic Reports

Open-ended record, so no end year. A search listing of Sweet & Maxwell's product page gave a 1971 date; that page was not fetched, so not used.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Road traffic reports | <https://sclqld.org.au/catalogue/records/19776> | Road traffic reports. |
| abbreviation | RTR / R.T.R. | <https://sclqld.org.au/catalogue/records/19776> | Variant title: RTR R.T.R. |
| start | 1970 | <https://sclqld.org.au/catalogue/records/19776> | Publication details: London : Kenneth Mason/Sweet Maxwell, 1970-. |

Not established: end; numbering by year, cited [YEAR] R.T.R. page.

### Crim. L.R. — Criminal Law Review

Monthly, Sweet & Maxwell. The record's Tübingen/New York 'Criminal law review' of 1954 is a different title and was not used.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Criminal law review | <https://sclqld.org.au/catalogue/records/20037> | Criminal law review. |
| abbreviation | Crim.L.R. / C.L.R. / Crim LR / Crim.L.Rev. | <https://sclqld.org.au/catalogue/records/20037> | Variant title: Crim.L.R. C.L.R. Crim LR Crim.L.Rev. |
| start | 1954 | <https://sclqld.org.au/catalogue/records/20037> | Publication details: London : Sweet Maxwell, 1954- |
| start | January 1954 | <https://en.wikipedia.org/wiki/The_Criminal_Law_Review> | The journal was established in January 1954. |
| end |  | <https://en.wikipedia.org/wiki/The_Criminal_Law_Review> | History Since 1954 |

Not established: numbering by year, cited [YEAR] Crim. L.R. page.

### conclusion — not a publication: 'Conclusion' is a table-of-contents heading followed by dot leaders and page numbers ('24 Conclusion.. 25').

### A.C.F. — Arrêts de la Cour fédérale du Canada (Federal Court Judgments, F.C.J.)

Laval's list pairs ACF with FCJ (a judgment database/service; its list has 'FCJ Federal Court Judgments'), not the Federal Courts Reports (listed there as F.C./F.C.R.; French R.C.F.). '[2005] 1 ACF 485' and '[2017] 1 ACF 167' carry a volume digit like a printed report, so they may be R.C.F. mis-read; not established. Australia's Federal Court Reports is not this.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Arrêts de la Cour fédérale du Canada = Federal Court of Canada Judgements (F.C.J) | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | ACF Arrêts de la Cour fédérale du Canada = Federal Court of Canada Judgements (F.C.J) |

Not established: numbering numbered judgments per year; '[2005] 1 ACF 485' has a volume and may be a different series; start.

### C. de D. — Les cahiers de droit

Open-ended record. Example (1977) vol 18 consistent with continuous numbering but arithmetic is not proof.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Les cahiers de droit | <https://sclqld.org.au/catalogue/records/72672> | Les cahiers de droit . |
| abbreviation | C. de D. and variants | <https://sclqld.org.au/catalogue/records/72672> | Variant title: Cah. droit Cahiers de droit C. de D. Cah.de Droit Cahiers Dr Cahiers Droit |
| start | 1954 | <https://sclqld.org.au/catalogue/records/72672> | Publication details: Québec : Faculté de droit, Université Laval, 1954- |
| numbering | continuous volumes, early volumes spanning two years (Vols. 1 1954-1955 to 7 1965-1966) | <https://sclqld.org.au/catalogue/records/72672> | Vols. 1 (1954-1955)-7 (1965-1966). |

### Int'l Lab. Rights Case L. — International Labor Rights Case Law

Brill \| Nijhoff. Example (2017), 3 matches Volume 3 (2017). Earlier file said volume 3 in 2017 'conflicts' with 5 in 2019; the Brill list gives Vol. 5 = 2019, Vol. 3 = 2017, consistent. Abbreviation 'Int'l Lab. Rights Case L.' not seen on a page; ISSNs 2405-688X (print), 2405-6901 (online).

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | International Labor Rights Case Law | <https://brill.com/view/journals/ilrc/ilrc-overview.xml> | International Labor Rights Case Law Annotated Jurisprudence on Fundamental Rights at the Workplace |
| start | 2015 | <https://brill.com/view/journals/ilrc/ilrc-overview.xml> | Volume: 1 (2015) |
| end |  | <https://brill.com/view/journals/ilrc/ilrc-overview.xml> | Volume: 12 (2026) |
| numbering | one volume per year, Vol. 1 = 2015 | <https://brill.com/view/journals/ilrc/ilrc-overview.xml> | Volume: 3 (2017) Show Items Volume: 2 (2016) Show Items Volume: 1 (2015) |

### Law Critique — Law and Critique

Springer's page returned a bot challenge. Wikipedia is secondary. 'Law Critique' as cited is not in the sources.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Law and Critique | <https://en.wikipedia.org/wiki/Law_and_Critique> | Law and Critique Discipline International law |
| abbreviation | Law Crit. (ISO 4) | <https://en.wikipedia.org/wiki/Law_and_Critique> | ISO 4 Law Crit. |
| start | 1990 | <https://en.wikipedia.org/wiki/Law_and_Critique> | It was established in 1990 |
| end |  | <https://en.wikipedia.org/wiki/Law_and_Critique> | History 1990–present |

Not established: numbering continuous volumes.

### A.S.I.L. Proc. — Proceedings of the American Society of International Law

Library records that would give 1907 and the later title 'Proceedings of the Annual Meeting' returned 403 or empty pages; no dates claimed. The title changed over time per search summaries (unfetched).

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Proceedings of the American Society of International Law | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | A.S.I.L.Proc. Proceedings of the American Society of International Law |

Not established: end; numbering continuous volumes (vol 98 = 2004 in the example); start.

### R.N.D.C. — National Journal of Constitutional Law

Earlier file said R.N.D.C. was unresolved; Laval's list resolves it. The catalogue record lists variant titles 'Natl. j. const. law Nat'l J. Const. L. Revue nationale de droit constitutionnel NJCL' and ISSN 1181-9340. Whether it still publishes is not stated.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | National Journal of Constitutional Law | <https://sclqld.org.au/catalogue/records/62964> | National journal of constitutional law = Revue nationale de droit constitutionnel. |
| abbreviation | R.N.D.C. | <https://www.bibl.ulaval.ca/explorer-par-discipline/droit/abreviations-juridiques> | R.N.D.C. Revue nationale de droit constitutionnel = National Journal of Constitutional Law |
| start | 1991 | <https://sclqld.org.au/catalogue/records/62964> | Publication details: Scarborough, Ontario : Carswell, 1991- |

Not established: end; numbering continuous volumes, more than one per year (vol 17 in 2004, 25 in 2008).

### B.C.L.R.B.D. No. — unresolved

Database decision-number identifier (year in square brackets + No.) for British Columbia Labour Relations Board decisions; no fetched page states the expansion of B.C.L.R.B.D. or its dates. Context fetched: the Board's own site cites decisions as 'BCLRB' (e.g. 2025 BCLRB 34) and says 'All decisions from January 1, 1990 are available electronically.' (https://www.lrb.bc.ca/decisions?page=42). Courthouse Libraries BC (https://www.courthouselibrary.ca/node/1863) lists print runs: '1974-1987 Decisions Labour Relations Board of British Columbia', '1987-1993 Decision of the Council British Columbia Industrial Relations Council', '1993-1999 British Columbia Labour Relations Board Decisions'. None names B.C.L.R.B.D. The 1974 example year coincides with the Board's establishment ('The LBR was established in 1974.') but that does not establish what the identifier covers.

Not established: abbreviation B.C.L.R.B.D. No.; name.

### C.L.R.B.R. — Canadian Labour Relations Board Reports

Cardiff (legalabbrevs.cardiff.ac.uk) returned HTTP 403 for every raw fetch, so 1974-1982, C.L.R.B.R., and the New Series (1983-1989) / Second Series (1989-) only come from search-result summaries, not fetched pages. The LexisNexis store page for 'Canadian Labour Relations Boards Reports, 3rd Series' came back as a JS shell with no '1974' text in the raw HTML, so the earlier 'since its initial publication in 1974' quote is not reproduced from a raw page here. Both example citations are 1974 so fall in the first series if the unverified dates hold.

Not established: end 1982; name Canadian Labour Relations Board Reports; numbering volumes numbered continuously within the series (examples cite '[1974] 1 Canadian LRBR'); successor New Series and Second Series restart; start 1974.

### Can. Bar Rev. — Canadian Bar Review

Kind journal, ongoing (no end). Abbreviation verified only in undotted form (2023 citation of vol. 101 issue 3). Quote for the French title is the page heading on the CBA site. Two of the examples pair a year with an implausible volume (e.g. '(1936), 20' and '(1942), 20') but this was not checked against a volume-by-year source, so no correction is claimed.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Canadian Bar Review | <https://cbr.cba.org/index.php/cbr/about> | Founded in 1923, The Canadian Bar Review is the bilingual peer-reviewed legal journal of the CBA |
| name_fr | Revue du Barreau canadien | <https://cbr.cba.org/index.php/cbr/article/view/4883> | REVUE DU BARREAU CANADIEN \| The Canadian Bar Review |
| start | 1923 | <https://www.uwindsor.ca/law/node/3322> | The first volume of the Canadian Bar Review (CBR) was published in 1923. |
| abbreviation | Can Bar Rev | <https://www.uwindsor.ca/law/node/3322> | (2023) 101:3 Can Bar Rev 478 |

Not established: numbering continuous volumes from vol. 1 (1923).

### B.C.H.R.T.D. No. — unresolved

Decision-number identifier for British Columbia Human Rights Tribunal decisions (Quicklaw-style). No fetched page uses 'B.C.H.R.T.D.'. Related fetched facts: Lexum neutral-citation table 'British Columbia Human Rights Tribunal (January 2000) Mazuelos v. Mary Jo Clark , 2000 BCHRT 1' (https://lexum.com/ccc-ccr/neutr/index_en.html) and Courthouse Libraries BC record: 'Print format ceased publication at end of 2009; continues to be published on the Internet. Cited as BCHRT.' (https://www.courthouselibrary.ca/catalogue-item?cid=6997). All four examples (2000-2011) are in the period when the neutral code BCHRT also existed.

Not established: abbreviation B.C.H.R.T.D. No..

### R.S.B.C. — not a publication: Revised Statutes of British Columbia 1960: '14 R.S.B.C. 1960' is section/chapter 14 of the statute revision, not a volume of a report.

Not a publication to register as a report/journal; a statute revision (statute series) form.

### S. — unresolved

Examples (11 S. 49, 54 S. 80, 60 S. 49, 149 S. 309) carry no year or context; nothing fetched identifies a series. Candidates (e.g. a Scottish Session Cases form) are unsupported and not claimed.

### R.T.S.N. — League of Nations Treaty Series

R.T.S.N. is the French-language abbreviation of the same series; that R.T.S.N. itself appears on any fetched page was NOT found (not verified). UN Treaty Collection page (https://treaties.un.org/Pages/Content.aspx?path=DB%2FLoNOnline%2FpageIntro_en.xml) states 'League of Nations Treaty Series (5 July 1920 - 3 October 1948)' for the span of treaties, and 'published in 205 volumes' - so the end year differs by source (1946 publication end per Wikipedia/Graduate Institute vol. 205 '1944/1946'; 1948 is the last treaty date per UN). Examples cite volumes 28, 52, 79, 85, all within vols 1-205.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Treaty Series: Treaties and International Engagements Registered with the Secretariat of the League of Nations | <https://libguides.graduateinstitute.ch/league_of_nations/treaty_series> | Treaty Series: Treaties and International Engagements Registered with the Secretariat of the League of Nations = Recueil des traités : traités et engagements internationaux enregistrés par le Secrétariat de la Société des Nations Vol. 1(192 |
| name_fr | Recueil des traités : traités et engagements internationaux enregistrés par le Secrétariat de la Société des Nations | <https://libguides.graduateinstitute.ch/league_of_nations/treaty_series> | Recueil des traités : traités et engagements internationaux enregistrés par le Secrétariat de la Société des Nations |
| abbreviation | LNTS (English abbreviation of the same series) | <https://en.wikipedia.org/wiki/League_of_Nations_Treaty_Series> | The League of Nations Treaty Series ( LNTS ) was a result of article 18 of the Covenant of the League of Nations |
| start | 1920 | <https://libguides.graduateinstitute.ch/league_of_nations/treaty_series> | Vol. 1(1920)-205(1944/1946, 9 indexes vol.) |
| end | 1946 | <https://en.wikipedia.org/wiki/League_of_Nations_Treaty_Series> | The LNTS began publication in 1920, and was terminated in 1946 |
| numbering | continuous volumes 1-205 (1920-1946) | <https://libguides.graduateinstitute.ch/league_of_nations/treaty_series> | Vol. 1(1920)-205(1944/1946, 9 indexes vol.) |

Not established: abbreviation R.T.S.N. (French abbreviation).

### CAF — Federal Court of Appeal

French neutral code of the Federal Court of Appeal; neutral citation began February 2001 (first-application month per Lexum).

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Federal Court of Appeal | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Federal Court of Appeal (February 2001) Bouvidard Ltée. v. Canada (Employment Insurance Commission) , 2001 FCA 1 |
| start | 2001 | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Federal Court of Appeal (February 2001) |
| abbreviation | CAF (French form of FCA) | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Bouvidard Ltée. c. Canada (Commission de l'assurance emploi) , 2001 CAF 1 |

Not established: name_fr Cour d'appel fédérale.

### Ll. L. Rep. — Lloyd's List Law Reports

Examples 1922 (vol. 12) and 1933 (vol. 46) fall in Lloyd's List Law Reports, 1919-1950. The 1999 example '[1999] 2 Ll.L. Rep 277' is after 1950 and so is not in the Ll L Rep series; the successor is Lloyd's Law Reports ('Lloyd's Rep'), which the ICLR page does not date and which is not verified here (a search summary of Cardiff says 1951/1968 differ; not fetched). Queensland record (https://sclqld.org.au/catalogue/records/19758): 'London : Lloyd's of London Press, 1919-' and variant title 'Lloyd's list law reports.' The 1999 citation likely uses the old abbreviation for the successor series.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Lloyd's List Law Reports | <https://www.iclr.co.uk/knowledge/guides/guide-to-uk-law-reports-and-their-citations> | Ll L Rep Lloyd’s List Law Reports (1919–1950) |
| abbreviation | Ll L Rep | <https://www.iclr.co.uk/knowledge/guides/guide-to-uk-law-reports-and-their-citations> | Ll L Rep Lloyd’s List Law Reports (1919–1950) |
| start | 1919 | <https://www.i-law.com/ilaw/doc/view.htm?id=134516> | (1919) 1 Ll L Rep 418 |
| end | 1950 | <https://www.iclr.co.uk/knowledge/guides/guide-to-uk-law-reports-and-their-citations> | Ll L Rep Lloyd’s List Law Reports (1919–1950) |

Not established: numbering continuous volumes, vol. 1 = 1919.

### C.P. — Law Reports, Common Pleas

Identification is by fit, not stated by the examples: the examples ('1 C.P. 274', '9 C.P. 208') have no year or prefix, volume numbers 1 and 9 fit vols 1-10, but another 'C.P.' series (e.g. an Upper Canada Common Pleas report) is not excluded. Treat as probable, with that caveat. Later series: Common Pleas Division 1875-1880 (per library record 'Continued by: Law reports. Common Pleas').

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | The law reports : Court of Common Pleas, 1865-1875 | <https://sclqld.org.au/catalogue/records/20120> | The law reports : Court of Common Pleas, 1865-1875. |
| abbreviation | L.R.C.P. / C.P. | <https://sclqld.org.au/catalogue/records/20120> | Variant title: Court of Common Pleas, 1865-1875 Common pleas, 1865-1875 L.R.C.P. LRCP C.P. Com.P.Div. |
| start | 1865 | <https://sclqld.org.au/catalogue/records/20120> | Online via ICLR online vol1 (1865) - vol10(1875) |
| end | 1875 | <https://sclqld.org.au/catalogue/records/20120> | Online via ICLR online vol1 (1865) - vol10(1875) |
| numbering | continuous volumes 1-10 (1865-1875) | <https://sclqld.org.au/catalogue/records/20120> | Online via ICLR online vol1 (1865) - vol10(1875) |

### CSC — Supreme Court of Canada

French neutral code; neutral citation began January 2000. The French court name was not on a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Supreme Court of Canada | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Supreme Court of Canada (January 2000) Arsenault-Cameron v. Prince Edward Island , 2000 SCC 1 |
| start | 2000 | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Supreme Court of Canada (January 2000) |
| abbreviation | CSC (French form of SCC) | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Arsenault-Cameron c. Île-du-Prince-Édouard , 2000 CSC 1 |

### background — not a publication: A heading/section word ('3 Background.. 3'), not a publication.

Heading or table-of-contents fragment.

### L.T. — Law Times Reports (New Series)

Dates are the library's holdings range (vol. 1 Nov 1859 - vol. 177 Dec 1947; record title says 'London : Law Times Office, 1860-1948' and 'Merged with: All England law reports ... to form: All England law reports'), so the end year 1947/1948 is the library record's, not an independent publisher statement. The record says 'Vols. 1-17 called new series, v. 1-4 also carry vol. designation of the "old series", v. 35-v. 38.' and that the series 'continues vols. 1-34 of The law times reports [old series]'. Volumes 18, 20, 29 (examples) have no year on the fetched record; no volume-year table was found, so no years for them are claimed. The numbering description is ambiguous in the record.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Law times reports | <https://sclqld.org.au/catalogue/records/19725> | Law times reports / Law times office. |
| abbreviation | L.T. / L.T.N.S. | <https://sclqld.org.au/catalogue/records/19725> | Law times reports (London, England : 1860) L.T. LT L.T.R. L.T.N.S. L.T.R.N.S. Law T.Rep.N.S. |
| start | 1859 | <https://sclqld.org.au/catalogue/records/19725> | Vol. 1 (Nov. 1859 to Mar. 1860) |
| end | 1947 | <https://sclqld.org.au/catalogue/records/19725> | vol. 177 (July to Dec. 1947) |
| numbering | volumes 1-177 consecutively (177 volumes); library says vols. 1-17 called new series | <https://sclqld.org.au/catalogue/records/19725> | Phys. description: 177 volumes |

### F.C.J. — unresolved

Federal Court (Canada) decision-number identifier on Quicklaw; the Justice Canada page shows '[1990] F.C.J. (Quicklaw) 234 (F.C.T.D.)'. The page does not expand F.C.J. and gives no dates. Earlier file marked the abbreviation 'contradicted' - that is not a contradiction: F.C.J. is the abbreviation, the Quicklaw label is a database tag.

| Fact | Value | Source | Quote |
|---|---|---|---|
| abbreviation | F.C.J. (Quicklaw) | <https://www.justice.gc.ca/eng/rp-pr/csj-sjc/harmonization/denau/tab_j.html> | Gervais v. Canada (M.N.R.), [1984] F.C.J. 1040 (Quicklaw) (F.C.T.D.). |

Not established: name.

### U.C.R. — Upper Canada Queen's Bench Reports, New Series

Queensland record (https://sclqld.org.au/catalogue/records/19899): 'Old Series: v. 1. 1830-1831 ... v. 6. 1838-1842' and 'New Series: v. 1. 1842-1845 ...'; the page also shows 'v. 43. 1878' region 'v. 42. 1877-1878 v. 43. 1878'. Example 10 U.C.R. 535 = New Series vol. 10 (1852-1853); 43 U.C.R. 460 = New Series vol. 43 (1878) (the Old Series has only 6 volumes). The abbreviation U.C.R. itself was not on a fetched page (Cardiff 403); the earlier file's 1844 start (a Cardiff snippet, never fetched) is contradicted by the two fetched records (1842). Quote for end/volumes copied from the stripped text of the record.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Upper Canada Queen’s Bench Reports, New Series | <https://digitalcommons.osgoode.yorku.ca/lawreports/4> | Upper Canada Queen’s Bench Reports, New Series, 1842 -1882 (46 v) |
| start | 1842 | <https://sclqld.org.au/catalogue/records/19899> | New Series: v. 1. 1842-1845 |
| end | 1882 | <https://sclqld.org.au/catalogue/records/19899> | v. 46. 1881-1882 |
| numbering | 46 continuous volumes (New Series), each with years | <https://sclqld.org.au/catalogue/records/19899> | v. 9. 1851-1852 v. 10. 1852-1853 |

Not established: abbreviation U.C.R..

### R.S.O. — not a publication: Revised Statutes of Ontario: '137 R.S.O. 1990' is chapter/section 137 of the 1990 revision, not a report volume.

Statute revision series.

### R.S.N.S. — not a publication: Revised Statutes of Nova Scotia 1967: leading number is a chapter, trailing year the revision.

Statute revision series.

### R.S.N. — not a publication: Revised Statutes of Newfoundland 1970: chapter number then revision year.

Statute revision series.

### R.S.A. — not a publication: Revised Statutes of Alberta 1970: chapter number then revision year.

Statute revision series.

### E.L.L.R. — Employment and Labour Law Reporter

Only Cardiff search summaries (title, E.L.L.R., 'Period : 1991-', ISSN 1183-7152, Canada); Cardiff returned 403 on every fetch and no other page was found. Nothing fetched confirms any fact.

Not established: abbreviation E.L.L.R.; name Employment and Labour Law Reporter; start 1991.

### SLR — Singapore Law Reports

NUS guide lists an earlier series too: 'The Singapore Law Reports Colony of Singapore Law Reports SLR 1950-51 (covers 1946-49) 1954-57 (covers 1953-56)'. All three examples (1992-1994) fall in the '1992 -' run of Singapore Law Reports. 'Singapore Law Reports (Reissue) SLR(R) 2010 (covers 1965 - 2009)' is a reissue. Not a Canadian series; cited in Canadian decisions as a foreign report. The volume-numbering scheme is not stated on a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Singapore Law Reports | <https://libguides.bodleian.ox.ac.uk/law-singapore/cases> | Law Report Citations Singapore Law Reports SLR Singapore Law Reports (Reissue) , cited SLR(R) |
| abbreviation | SLR | <https://libguides.bodleian.ox.ac.uk/law-singapore/cases> | Law Report Citations Singapore Law Reports SLR Singapore Law Reports (Reissue) , cited SLR(R) |
| start | 1965 | <https://libguides.nus.edu.sg/SGlawhistory/cases> | *Singapore Law Reports SLR 1965-91 1992 - |

Not established: numbering by year from 1992 (examples '[1992] 2 SLR 495').

### C.R.C. — Canadian Railway Cases

Published originally by Canada Law Book (page: 'While this was published originally by Canada Law Book'). Dotted C.R.C. not on the page. The citation form 'C.R.C. 1978' with a chapter is the unrelated Consolidated Regulations of Canada; the examples here have volume-then-page so are the cases series.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Canadian Railway Cases | <https://robesideassistance.ca/2017/07/12/asked-and-answered-canadian-railway-cases/> | The citation “CRC” refers to a dusty, but clearly still useful, law report series titled Canadian Railway Cases . |
| start | 1902 | <https://robesideassistance.ca/2017/07/12/asked-and-answered-canadian-railway-cases/> | The series ran from 1902 to 1939 |
| end | 1939 | <https://robesideassistance.ca/2017/07/12/asked-and-answered-canadian-railway-cases/> | The series ran from 1902 to 1939 |
| abbreviation | CRC | <https://robesideassistance.ca/2017/07/12/asked-and-answered-canadian-railway-cases/> | The citation is 1 CRC 461. |

Not established: numbering continuous volumes (example '1 CRC 461').

### Int. Lawyer — The International Lawyer

Publisher per the MARC record: 'Section of International Law, American Bar Association'. End not checked (ABA and JSTOR pages did not load). The catalogue does not say whether volumes are continuous or one per year.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | The International Lawyer | <https://librarycatalog.ecu.edu/catalog/3894087> | The International lawyerh\| [electronic resource]. |
| start | 1966 | <https://librarycatalog.ecu.edu/catalog/3894087> | Began with vol. 1, no. 1 (Oct. 1966). |

Not established: abbreviation Int. Lawyer; numbering vol. 1 = 1966; quarterly.

### ONCA — Court of Appeal for Ontario

Neutral code; neutral citation began January 2007 per Lexum. The Lexum page names it 'Ontario Court of Appeal'; 'Court of Appeal for Ontario' is not on that page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Ontario Court of Appeal | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Ontario Court of Appeal (January 2007) Atec Marketing Limited v. Heart and Stroke Foundation of Canadan , 2007 ONCA 1 |
| start | 2007 | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Ontario Court of Appeal (January 2007) |

Not established: name_fr Cour d'appel de l'Ontario.

### C.L. — unresolved

Single example '11 C.L. 30' with no year; nothing fetched identifies it.

### CHRT — Canadian Human Rights Tribunal

Tribunal's neutral citation began January 2003 per Lexum. French code is TCDP, not CHRT.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | Canadian Human Rights Tribunal | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Canadian Human Rights Tribunal (January 2003) Desormeaux v. Commission de transport régionale d'Ottawa-Carleton , 2003 CHRT 2 |
| start | 2003 | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Canadian Human Rights Tribunal (January 2003) |
| abbreviation | TCDP (French code) | <https://lexum.com/ccc-ccr/neutr/index_en.html> | Desormeaux c. Commission de transport régionale d'Ottawa-Carleton , 2003 TCDP 2 |

Not established: name_fr Tribunal canadien des droits de la personne.

### G.S.T.C. — unresolved

Used in Canadian case citations for GST decisions (search summary shows '[2010] G.S.T.C. 186'), probably a Carswell/Thomson Reuters Canada reporter, but no raw page fetched states the name, publisher or dates (the KPMG PDF could not be read as text).

Not established: abbreviation G.S.T.C.; name.

### OLRB Rep. — OLRB Reports

Same series as olrbrepseptember: the month ('April') is the issue, so the registry entry is one series 'OLRB Rep.' with issue months. The Board page does not give a start year (a search summary mentioned 1944 for decisions online, not fetched). Example [1985] OLRB Rep. April 594 is a year-based citation.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | OLRB Reports (Ontario Labour Relations Board) | <https://www.olrb.gov.on.ca/OLRBReportsRequest-EN.asp> | The Ontario Labour Relations Board publishes a bi-monthly collection of significant decisions. The OLRB Reports are one of the oldest reporting series for labour adjudication in North America. |

Not established: numbering bi-monthly issues, month named in citation; volume by year; start.

### OLRB Rep. — OLRB Reports

Same series as olrbrepapril; 'September' is an issue month, not part of the title. Example [1993] OLRB Rep. September 873.

| Fact | Value | Source | Quote |
|---|---|---|---|
| name | OLRB Reports (Ontario Labour Relations Board) | <https://www.olrb.gov.on.ca/OLRBReportsRequest-EN.asp> | The Ontario Labour Relations Board publishes a bi-monthly collection of significant decisions. The OLRB Reports are one of the oldest reporting series for labour adjudication in North America. |

Not established: start.

### Act — not a publication: A year, the word Act and a number ('1993 Act 459') - a statute citation shape/heading word, not a publication.

Not a publication.

## Journal runs and volume anchors

### canbarrev — The Canadian Bar Review = La revue du barreau canadien

Journal's own OJS archive (cbr.cba.org) pages 1-24; older volumes have up to 10 numbers a year. The bilingual title form appears in the catalogue record; the date it was adopted was not found on a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1923 | <https://ci.nii.ac.jp/ncid/AA00116510> | Vol. 1 (1923)- |
| current | True | <https://cbr.cba.org/index.php/cbr/issue/archive> | Vol. 104 No. 2 (2026) |
| volume spans two years (vol 93 = 2015 and 2016) | vol 93 2015-2016 | <https://cbr.cba.org/index.php/cbr/issue/archive/2> | Vol. 93 No. 3 (2016) Vol. 93 No. 2 (2015) Vol. 93 No. 1 (2015) |
| several volumes in one year (2016: vols 93 and 94) | 2016 | <https://cbr.cba.org/index.php/cbr/issue/archive/2> | Vol. 94 No. 1 (2016) Vol. 93 No. 3 (2016) |
| frequency | 10 times a year 1923-1957; quarterly 1958- | <https://ci.nii.ac.jp/ncid/AA00116510> | 10 times a year, 1923-1957; quarterly, 1958- |
| incorporated titles | Canada law journal; Canadian law times | <https://ci.nii.ac.jp/ncid/AA00116510> | With which are incorporated the Canada law journal and the Canadian law times |
| vol. 1 | 1923 | <https://cbr.cba.org/index.php/cbr/issue/archive/24> | Vol. 1 No. 3 (1923) |
| vol. 17 | 1939 | <https://cbr.cba.org/index.php/cbr/issue/archive/18> | Vol. 17 No. 1 (1939) |
| vol. 38 | 1960 | <https://cbr.cba.org/index.php/cbr/issue/archive/10> | Vol. 38 No. 1 (1960) |
| vol. 75 | 1996 | <https://cbr.cba.org/index.php/cbr/issue/archive/4> | Vol. 75 No. 1 (1996) |
| vol. 104 | 2026 | <https://cbr.cba.org/index.php/cbr/issue/archive> | Vol. 104 No. 2 (2026) |
| title | The Canadian Bar Review = La revue du barreau canadien 1923- | <https://ci.nii.ac.jp/ncid/AA00116510> | The Canadian bar review = La revue du barreau canadien Canadian Bar Association, 1923- |

### harvlrev — Harvard Law Review

Wikisource (not the journal) gives academic-year spans for vols 1-12 and 32-35; the anchor year is the first year of the span. The journal's own explore-by-volume page lists volumes without years; its print page gives Vol. 139 No. 8 June 2026.

| Fact | Value | Source | Quote |
|---|---|---|---|
| volumes span two calendar years | Volume 1 (1887-88), Volume 2 (1888-89) | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 1 (1887–88) ( transcription project ) Volume 2 (1888-89) |
| current | True | <https://harvardlawreview.org/print/> | Vol. 139 No. 8 June 2026 |
| journal site copyright range | 1887-2026 | <https://harvardlawreview.org/about/> | Copyright © 1887-2026 Harvard Law Review |
| vol. 1 | 1887 | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 1 (1887–88) |
| vol. 4 | 1890 | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 4 (1890-91) |
| vol. 12 | 1898 | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 12 (1898-99) |
| vol. 32 | 1918 | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 32 (1918-19) |
| vol. 35 | 1921 | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 35 (1921-22) |
| vol. 139 | 2026 | <https://harvardlawreview.org/print/> | Vol. 139 No. 8 June 2026 |
| title | Harvard Law Review 1887- | <https://en.wikisource.org/wiki/Harvard_Law_Review> | Volume 1 (1887–88) |

### mcgilllj — McGill Law Journal / Revue de droit de McGill

Both fetched pages are the journal's own. They differ on the vol 1/2 spans (issues page: vol 1 = 1952-54, vol 2 = 1955-56; preface: first two volumes together ran 1952-1957). Registry examples (1969,15), (1973,19), (1975,21), (1981,26) agree with the issues page (15=1969, 19=1973, 21=1975, 26=1980-81).

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1952 | <https://lawjournal.mcgill.ca/article/prefece-to-the-twenty-fifth-anniversary-volume/> | when the first number of the first volume appear- ed in the autumn of 1952 |
| two volumes in 1967 | Volume 12 1966-67; Volume 13 1967 | <https://lawjournal.mcgill.ca/issues/> | Volume 12 1966-67 No1 No2 No3 No4 Volume 13 1967 |
| two volumes in 2010 | Volume 55 2010; Volume 56 2010-11 | <https://lawjournal.mcgill.ca/issues/> | Volume 55 2010 No1 No2 No3 No4 Volume 56 2010-11 |
| current | True | <https://lawjournal.mcgill.ca/issues/> | Volume 71 2026 No1 No2 No3 |
| vol. 1 | 1952 | <https://lawjournal.mcgill.ca/issues/> | Volume 1 1952-54 |
| vol. 10 | 1964 | <https://lawjournal.mcgill.ca/issues/> | Volume 10 1964 |
| vol. 26 | 1980 | <https://lawjournal.mcgill.ca/issues/> | Volume 26 1980-81 |
| vol. 43 | 1998 | <https://lawjournal.mcgill.ca/issues/> | Volume 43 1998 |
| vol. 71 | 2026 | <https://lawjournal.mcgill.ca/issues/> | Volume 71 2026 |
| title | McGill Law Journal 1952- | <https://lawjournal.mcgill.ca/article/prefece-to-the-twenty-fifth-anniversary-volume/> | when the first number of the first volume appear- ed in the autumn of 1952 |
| title | also known as Revue de droit de McGill 1977- | <https://lawjournal.mcgill.ca/article/prefece-to-the-twenty-fifth-anniversary-volume/> | known as well since 1977 as the Revue de Droit de McGill |

Not established: first two volumes span (preface) 1952-1957 across two volumes; volume 1 span (issues page) 1952-54.

### utlj — University of Toronto Law Journal

Anchors are from the Crossref API (volume + issued date per article); the journal's own site (utpjournals.press) and JSTOR blocked raw fetch. Crossref lists vol 76 for 2026 as the latest. Crossref display title is "The University of Toronto Law Journal" up to 2000 and "University of Toronto Law Journal" in 2025; the change date was not pinned.

| Fact | Value | Source | Quote |
|---|---|---|---|
| established | 1935 | <https://en.wikipedia.org/wiki/University_of_Toronto_Law_Journal> | The journal was established in 1935 |
| current | True | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:2025,until-pub-date:2025&rows=100&select=volume,issued,container-title> | {"volume":"75","issued":{"date-parts":[[2025,10,1]]},"container-title":["University of Toronto Law Journal"]} |
| volume 1 appears under both 1935 and 1936 in Crossref | vol 1 spans 1935-36 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:1936,until-pub-date:1936&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1936]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 1 | 1935 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:1935,until-pub-date:1935&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1935]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 8 | 1950 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:1950,until-pub-date:1950&rows=100&select=volume,issued,container-title> | {"volume":"8","issued":{"date-parts":[[1950]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 25 | 1975 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:1975,until-pub-date:1975&rows=100&select=volume,issued,container-title> | {"volume":"25","issued":{"date-parts":[[1975]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 30 | 1980 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:1980,until-pub-date:1980&rows=100&select=volume,issued,container-title> | {"volume":"30","issued":{"date-parts":[[1980]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 50 | 2000 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:2000,until-pub-date:2000&rows=100&select=volume,issued,container-title> | {"volume":"50","issued":{"date-parts":[[2000]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 75 | 2025 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:2025,until-pub-date:2025&rows=100&select=volume,issued,container-title> | {"volume":"75","issued":{"date-parts":[[2025,10,1]]},"container-title":["University of Toronto Law Journal"]} |
| title | The University of Toronto Law Journal 1935-2000 | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:1935,until-pub-date:1935&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1935]]},"container-title":["The University of Toronto Law Journal"]} |
| title | University of Toronto Law Journal 2025- | <https://api.crossref.org/journals/0042-0220/works?filter=from-pub-date:2025,until-pub-date:2025&rows=100&select=volume,issued,container-title> | {"volume":"75","issued":{"date-parts":[[2025,10,1]]},"container-title":["University of Toronto Law Journal"]} |

Not established: finding aid Vols. 1-5 (1935-44); frequency Quarterly, 1968-; start (vol 1 Lent term 1935) 1935.

### osgoodehalllj — Osgoode Hall Law Journal

Journal's own Digital Commons all-issues list. Early volumes ran several years (vol 2 = April 1960 to April 1963, vol 3 = 1964-65); later volumes run an academic year (vol 49 Fall 2011 to Summer 2012). Earlier title not verified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1958 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 1, Number 1 (June 1958) |
| volume 1 also dated 1959 | Volume 1 (April 1959) | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 1, Number 2 (April 1959) |
| volume 2 spans 1960-1963 | Volume 2 issues April 1960, 1961, 1962, 1963 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 2, Number 4 (April 1963) |
| two volumes in 2012 | Volume 49 Summer 2012; Volume 50 Winter 2012 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 49, Number 3 (Summer 2012) |
| two volumes in 2012 (second) | Volume 50 Winter 2012 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 50, Issue 2 (Winter 2012) |
| current | True | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 63, Number 1 (Fall 2026) |
| continuous since | 1958 | <https://digitalcommons.osgoode.yorku.ca/ohlj/> | Publishing continuously since 1958 |
| vol. 1 | 1958 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 1, Number 1 (June 1958) |
| vol. 2 | 1960 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 2, Number 1 (April 1960) |
| vol. 12 | 1974 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 12, Number 3 (December 1974) |
| vol. 30 | 1992 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 30, Number 1 (Spring 1992) |
| vol. 50 | 2013 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 50, Issue 3 (Spring 2013) |
| vol. 63 | 2026 | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 63, Number 1 (Fall 2026) |
| title | Osgoode Hall Law Journal 1958- | <https://digitalcommons.osgoode.yorku.ca/ohlj/all_issues.html> | Volume 1, Number 1 (June 1958) |

### yalelj — The Yale Law Journal

Crossref API only (yalelawjournal.org returned a block page; Crossref has no Yale items after 2008). Start 1891 is the issued year of volume 1 items in Crossref, not an explicit statement. Registry example (1949) 58:8 is consistent: vol 59 is 1950.

| Fact | Value | Source | Quote |
|---|---|---|---|
| two volumes issued in 1980 | vol 89 and vol 90 (quotes: vol 90 item shown; vol 89 is anchor) | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:1980,until-pub-date:1980&rows=100&select=volume,issued,container-title> | {"volume":"90","issued":{"date-parts":[[1980,12]]},"container-title":["The Yale Law Journal"]} |
| vol. 1 | 1891 | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:1891,until-pub-date:1891&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1891,10]]},"container-title":["The Yale Law Journal"]} |
| vol. 28 | 1919 | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:1919,until-pub-date:1919&rows=100&select=volume,issued,container-title> | {"volume":"28","issued":{"date-parts":[[1919,2]]},"container-title":["The Yale Law Journal"]} |
| vol. 59 | 1950 | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:1950,until-pub-date:1950&rows=100&select=volume,issued,container-title> | {"volume":"59","issued":{"date-parts":[[1950,2]]},"container-title":["The Yale Law Journal"]} |
| vol. 89 | 1980 | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:1980,until-pub-date:1980&rows=100&select=volume,issued,container-title> | {"volume":"89","issued":{"date-parts":[[1980,4]]},"container-title":["The Yale Law Journal"]} |
| vol. 118 | 2008 | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:2008,until-pub-date:2008&rows=100&select=volume,issued,container-title> | {"volume":"118","issued":{"date-parts":[[2008,10,1]]},"container-title":["The Yale Law Journal"]} |
| title | The Yale Law Journal 1891- | <https://api.crossref.org/journals/0044-0094/works?filter=from-pub-date:1891,until-pub-date:1891&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1891,10]]},"container-title":["The Yale Law Journal"]} |

Not established: current.

### queenslj — Queen's Law Journal

Wikipedia (fetched) states 1968 founding as Queen's Intramural Law Journal and the 1971 title change; it does not state the volume 1 year. Catalogue pages with vol 1 (1971) blocked the raw fetch, so start stays null. Journal's own issues page lists vols 38-51 only. Registry examples with vols 3-10 (1977-85) could not be checked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| predecessor start | 1968 | <https://en.wikipedia.org/wiki/Queen%27s_Law_Journal> | established in 1968 as the Queen's Intramural Law Journal |
| volumes span two calendar years | Volume 46: Fall 2020 and Spring 2021 | <https://journal.queenslaw.ca/issues> | Volume 46, No. 2, Spring 2021 |
| current | True | <https://journal.queenslaw.ca/issues> | Volume 51, No. 2, Winter 2026 |
| vol. 38 | 2013 | <https://journal.queenslaw.ca/issues> | Volume 38, No. 2, Spring 2013 |
| vol. 40 | 2014 | <https://journal.queenslaw.ca/issues> | Volume 40, No. 1, Fall 2014 |
| vol. 46 | 2020 | <https://journal.queenslaw.ca/issues> | Volume 46, No. 1, Fall 2020 |
| vol. 51 | 2026 | <https://journal.queenslaw.ca/issues> | Volume 51, No. 2, Winter 2026 |
| title | Queen's Intramural Law Journal 1968-1971 | <https://en.wikipedia.org/wiki/Queen%27s_Law_Journal> | established in 1968 as the Queen's Intramural Law Journal |
| title | Queen's Law Journal 1971- | <https://en.wikipedia.org/wiki/Queen%27s_Law_Journal> | obtaining its current title in 1971 |

Not established: volume 1 year 1971.

### sclr2d — The Supreme Court Law Review, Second Series

Only a Japanese union catalogue record (CiNii) was fetched. It gives "Vol. 1 (1990)" in the bibliographic field but "1991-2024" in its metadata. Whether the series ceased after vol 115 (2024) is not shown. Registry example vol 21 (2003) next to catalogue vol 19 (2003) implies several volumes in 2003; no page lists them. Registry examples (2004,25),(2007,36),(2008,40) not checked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| last volume held in catalogue | v. 115 (2024) | <https://ci.nii.ac.jp/ncid/AA10819357> | Vol. 1 (1990)-v. 115 (2024) |
| frequency | annual 1990-1999; semiannual 2001, 2024; approximately quarterly 2002-2023 | <https://ci.nii.ac.jp/ncid/AA10819357> | Frequency varies: annual, 1990-1999; semiannual, 2001, 2024; approximately quarterly, 2002-2023 |
| vol. 1 | 1990 | <https://ci.nii.ac.jp/ncid/AA10819357> | Vol. 1 (1990)-v. 115 (2024) |
| vol. 19 | 2003 | <https://ci.nii.ac.jp/ncid/AA10819357> | Vol. 19 (2003)- published by LexisNexis |
| title | The Supreme Court law review. Second series 1990- | <https://ci.nii.ac.jp/ncid/AA10819357> | Vol. 1 (1990)-v. 115 (2024) |

Not established: catalogue metadata date span 1991-2024; end 2024; predecessor series Supreme Court Law Review (Butterworths) 1980-1989.

### altalrev — Alberta Law Review

Journal's own OJS archive lists vol/no with few years; only 62/2025 appears paired. Crossref supplies 14/1976, 34 and 35/1996, 48/2010. Crossref has many Alberta items with empty volume; earliest is dated 1955 without a volume, so start stays null. Registry example (1964) 3:2 could not be checked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| volumes 34 and 35 both in 1996 | vol 35 (vol 34 is anchor) | <https://api.crossref.org/journals/0002-4821/works?filter=from-pub-date:1996,until-pub-date:1996&rows=100&select=volume,issued,container-title> | {"volume":"35","issued":{"date-parts":[[1996,11,1]]},"container-title":["Alberta Law Review"]} |
| current | True | <https://albertalawreview.com/index.php/ALR/issue/archive> | Vol 64, No 1 Vol 63, No 4 2026 |
| vol. 14 | 1976 | <https://api.crossref.org/journals/0002-4821/works?filter=from-pub-date:1976,until-pub-date:1976&rows=100&select=volume,issued,container-title> | {"volume":"14","issued":{"date-parts":[[1976,3,1]]},"container-title":["Alberta Law Review"]} |
| vol. 34 | 1996 | <https://api.crossref.org/journals/0002-4821/works?filter=from-pub-date:1996,until-pub-date:1996&rows=100&select=volume,issued,container-title> | {"volume":"34","issued":{"date-parts":[[1996,2,1]]},"container-title":["Alberta Law Review"]} |
| vol. 48 | 2010 | <https://api.crossref.org/journals/0002-4821/works?filter=from-pub-date:2010,until-pub-date:2010&rows=100&select=volume,issued,container-title> | {"volume":"48","issued":{"date-parts":[[2010,12,1]]},"container-title":["Alberta Law Review"]} |
| vol. 62 | 2025 | <https://albertalawreview.com/index.php/ALR/issue/archive> | Vol 62, No 4 2025 |
| title | Alberta Law Review None- | <https://api.crossref.org/journals/0002-4821/works?filter=from-pub-date:1976,until-pub-date:1976&rows=100&select=volume,issued,container-title> | {"volume":"14","issued":{"date-parts":[[1976,3,1]]},"container-title":["Alberta Law Review"]} |

Not established: earliest Crossref item 1955.

### ubclrev — UBC Law Review

Journal's own Digital Commons lists vols 44-59 (2011-2026), one per year (several_volumes false only for that range). Older volumes not online. Volume-1 year and how numbering relates to 1959 not found; registry example (1967) vol 3 is unchecked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| predecessor start | 1949 | <https://historyproject.allard.ubc.ca/law-history-project/milestone/founding-ubc-law-review> | first published in 1949 as a collection of student essays entitled the UBC Legal Notes |
| became UBC Law Review | 1959 | <https://historyproject.allard.ubc.ca/law-history-project/milestone/founding-ubc-law-review> | In 1959, it officially became the UBC Law Review |
| catalogue publication date | 1959 | <https://courthouselibrary.ca/catalogue-item?cid=8941> | Publication Date: 1959 |
| vol. 44 | 2011 | <https://commons.allard.ubc.ca/ubclawreview/all_issues.html> | Volume 44 Issue 1 (2011) |
| vol. 48 | 2015 | <https://commons.allard.ubc.ca/ubclawreview/all_issues.html> | Volume 48 Issue 1 (2015) |
| vol. 52 | 2019 | <https://commons.allard.ubc.ca/ubclawreview/all_issues.html> | Volume 52 Issue 1 (2019) |
| vol. 59 | 2026 | <https://commons.allard.ubc.ca/ubclawreview/all_issues.html> | Volume 59 Issue 1 (2026) |
| title | UBC Legal Notes 1949-1959 | <https://historyproject.allard.ubc.ca/law-history-project/milestone/founding-ubc-law-review> | first published in 1949 as a collection of student essays entitled the UBC Legal Notes |
| title | UBC Law Review 1959- | <https://historyproject.allard.ubc.ca/law-history-project/milestone/founding-ubc-law-review> | In 1959, it officially became the UBC Law Review |

Not established: volume 1 year.

### njcl — National Journal of Constitutional Law

No page for this journal could be fetched raw (catalogue hosts returned block pages; Cardiff 403). Search-result snippets mention vol 1 no 1 March 1991 and Carswell; none confirmed by a fetched page. Registry examples (1993) 3, (1997) 7, (1998) 9, (2000) 11 and 12 (two volumes in 2000) unchecked.

Not established: publisher Carswell, Scarborough; start (vol 1 no 1 March 1991) 1991.

### crimlq — Criminal Law Quarterly

Only a bookseller listing (Wildy) was fetched; start is its marketing text, release dates are per-part. Volumes 72 and 73 are labelled with two years (parts in year Y, bound volume released in Y+1). No early anchors fetched. Registry examples (1963) 5, (1971) 14, (1980-81) 23, (1987) 29, (1993) 35, (1998) 40 not checked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1958 | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | has been connecting criminal law practitioners to important issues and cases since 1958 |
| current | True | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | Volume 74 Number 3 2026 September 22, 2026 |
| volume 73 labelled 2025 and 2026 (bound volume 2026) | vol 73 | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | Volume 73 Number Bound Volume 2026 January 12, 2026 |
| vol 72 labelled 2024 and 2025 | vol 72 | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | Volume 72 Number Bound Volume 2025 January 16, 2025 |
| vol. 72 | 2024 | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | Volume 72 Number 3 2024 |
| vol. 73 | 2025 | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | Volume 73 Number 3 2025 |
| vol. 74 | 2026 | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | Volume 74 Number 3 2026 |
| title | Criminal Law Quarterly 1958- | <https://wildy.com/isbn/0011-1333/criminal-law-quarterly-journal-canada-law-book> | has been connecting criminal law practitioners to important issues and cases since 1958 |

### lawqrev — Law Quarterly Review

Only Wikipedia fetched (infobox History 1885-present). No archive page with volume/year pairs could be fetched (Westlaw UK, HeinOnline, JSTOR not reachable raw). Registry examples (1892) 8, (1930) 46, (1939) 55, (1962) 78 unchecked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1885 | <https://en.wikipedia.org/wiki/Law_Quarterly_Review> | History 1885–present |
| title | Law Quarterly Review 1885- | <https://en.wikipedia.org/wiki/Law_Quarterly_Review> | History 1885–present |

Not established: anchors; volume 1 year 1885.

### dallj — Dalhousie Law Journal

Journal's own Digital Commons all-issues page gives the year of issue 1 of each volume. Gaps show vols 8 and 9 both starting in 1984, vol 11 in 1987 and 12 in 1989 (no 1988). Registry example (1983) 7 cannot be placed: vol 7 issue 1 is 1982. Earlier title not checked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1973 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 1 Issue 1 (1973) |
| two volumes issue 1 in 1984 | Volume 8 and Volume 9 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 9 Issue 1 (1984) |
| two volumes issue 1 in 1984 (vol 8) | Volume 8 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 8 Issue 1 (1984) |
| current | True | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 49 Issue 1 (2026) |
| vol. 1 | 1973 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 1 Issue 1 (1973) |
| vol. 13 | 1990 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 13 Issue 1 (1990) |
| vol. 25 | 2002 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 25 Issue 1 (2002) |
| vol. 38 | 2015 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 38 Issue 1 (2015) |
| vol. 49 | 2026 | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 49 Issue 1 (2026) |
| title | Dalhousie Law Journal 1973- | <https://digitalcommons.schulichlaw.dal.ca/dlj/all_issues.html> | Volume 1 Issue 1 (1973) |

### columlrev — Columbia Law Review

Crossref API (volume + issued); journal site blocked raw fetch and Crossref has nothing after 2004 for this ISSN, so current status is unproven. Wikipedia infobox says 1901-present.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start per Wikipedia infobox | 1901 | <https://en.wikipedia.org/wiki/Columbia_Law_Review> | History 1901–present |
| vol. 1 | 1901 | <https://api.crossref.org/journals/0010-1958/works?filter=from-pub-date:1901,until-pub-date:1901&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1901,12]]},"container-title":["Columbia Law Review"]} |
| vol. 47 | 1947 | <https://api.crossref.org/journals/0010-1958/works?filter=from-pub-date:1947,until-pub-date:1947&rows=100&select=volume,issued,container-title> | {"volume":"47","issued":{"date-parts":[[1947,1]]},"container-title":["Columbia Law Review"]} |
| vol. 80 | 1980 | <https://api.crossref.org/journals/0010-1958/works?filter=from-pub-date:1980,until-pub-date:1980&rows=100&select=volume,issued,container-title> | {"volume":"80","issued":{"date-parts":[[1980,5]]},"container-title":["Columbia Law Review"]} |
| vol. 104 | 2004 | <https://api.crossref.org/journals/0010-1958/works?filter=from-pub-date:2004,until-pub-date:2004&rows=100&select=volume,issued,container-title> | {"volume":"104","issued":{"date-parts":[[2004,6]]},"container-title":["Columbia Law Review"]} |
| title | Columbia Law Review 1901- | <https://api.crossref.org/journals/0010-1958/works?filter=from-pub-date:1901,until-pub-date:1901&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1901,12]]},"container-title":["Columbia Law Review"]} |

Not established: current.

### unblj — University of New Brunswick Law Journal

Journal's own OJS archive (2 pages) lists vols 1-76. Vol 1 appears under 1947 and 1948; vols 5 and 6 both in 1952; vols 60 and 61 both in 2010; vols 3 and 4 both under 1950. Journal title wording on those pages was not extracted, so title_history is empty. Registry examples (1978,27),(1984,33),(1989,38),(1990,39),(1995,44),(1996,45) agree with the archive.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1947 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 1 No. 1 (1947) |
| volume 1 also listed in 1948 | Vol. 1 No. 2 (1948) | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 1 No. 2 (1948) |
| two volumes in 1952 | Vol 5 and Vol 6 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 6 No. 1 (1952) |
| two volumes in 1952 (vol 5) | Vol 5 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 5 No. 2 (1952) |
| two volumes in 2010 (vol 61) | Vol 61 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive> | Vol. 61 (2010) |
| two volumes in 2010 (vol 60) | Vol 60 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive> | Vol. 60 (2010) |
| current | True | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive> | Vol. 76 (2025) |
| vol. 1 | 1947 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 1 No. 1 (1947) |
| vol. 13 | 1961 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 13 (1961) |
| vol. 27 | 1978 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive/2> | Vol. 27 (1978) |
| vol. 45 | 1996 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive> | Vol. 45 (1996) |
| vol. 76 | 2025 | <https://journals.lib.unb.ca/index.php/UNBLJ/issue/archive> | Vol. 76 (2025) |

### modlrev — The Modern Law Review

Crossref API (volume + issued); Wiley/MLR site blocked. Vol 1 = 1937, vol 2 spans 1938-39, vol 88 in 2025 and vol 89 in both 2025/2026 range. Registry example (1939) 3 and (1955) 18 match.

| Fact | Value | Source | Quote |
|---|---|---|---|
| vol 1 and vol 2 both in 1938 | 1938 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1938,until-pub-date:1938&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1938,3]]},"container-title":["The Modern Law Review"]} |
| vol 2 also dated 1938 (companion) | vol 2 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1938,until-pub-date:1938&rows=100&select=volume,issued,container-title> | {"volume":"2","issued":{"date-parts":[[1938,9]]},"container-title":["The Modern Law Review"]} |
| vol 2 also in 1939 | vol 2 in 1938 and 1939 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1939,until-pub-date:1939&rows=100&select=volume,issued,container-title> | {"volume":"2","issued":{"date-parts":[[1939,3]]},"container-title":["The Modern Law Review"]} |
| current (vol 89 in 2026) | True | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:2026,until-pub-date:2026&rows=100&select=volume,issued,container-title> | {"volume":"89","issued":{"date-parts":[[2026,4,5]]},"container-title":["The Modern Law Review"]} |
| vol. 1 | 1937 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1937,until-pub-date:1937&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1937,6]]},"container-title":["The Modern Law Review"]} |
| vol. 3 | 1939 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1939,until-pub-date:1939&rows=100&select=volume,issued,container-title> | {"volume":"3","issued":{"date-parts":[[1939,6]]},"container-title":["The Modern Law Review"]} |
| vol. 18 | 1955 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1955,until-pub-date:1955&rows=100&select=volume,issued,container-title> | {"volume":"18","issued":{"date-parts":[[1955,11]]},"container-title":["The Modern Law Review"]} |
| vol. 43 | 1980 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1980,until-pub-date:1980&rows=100&select=volume,issued,container-title> | {"volume":"43","issued":{"date-parts":[[1980,3]]},"container-title":["The Modern Law Review"]} |
| vol. 88 | 2025 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:2025,until-pub-date:2025&rows=100&select=volume,issued,container-title> | {"volume":"88","issued":{"date-parts":[[2025,2,24]]},"container-title":["The Modern Law Review"]} |
| vol. 89 | 2026 | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:2026,until-pub-date:2026&rows=100&select=volume,issued,container-title> | {"volume":"89","issued":{"date-parts":[[2026,4,5]]},"container-title":["The Modern Law Review"]} |
| title | The Modern Law Review 1937- | <https://api.crossref.org/journals/0026-7961/works?filter=from-pub-date:1937,until-pub-date:1937&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1937,6]]},"container-title":["The Modern Law Review"]} |

### windsorybaccessjust — Windsor Yearbook of Access to Justice

Journal's own OJS archive back to vol 25 (2007) only; earlier volumes (registry examples (1984) 4, (1998) 16, (2003) 22) not on a fetched page. Crossref returned items with wrong/odd dates for this ISSN (vol 33 dated 1969), so Crossref was not used. several_volumes_some_year false only within vols 25-41.

| Fact | Value | Source | Quote |
|---|---|---|---|
| current | True | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive> | Vol. 41 (2025) |
| gap: no volume in 2014 | Vol 31 (2013) then Vol 32 (2015) | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive> | Vol. 32 No. 1 (2015) |
| vol. 25 | 2007 | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive/2> | Vol. 25 No. 2 (2007) |
| vol. 28 | 2010 | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive> | Vol. 28 No. 1 (2010) |
| vol. 31 | 2013 | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive> | Vol. 31 No. 1 (2013) |
| vol. 36 | 2019 | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive> | Vol. 36 (2019) |
| vol. 41 | 2025 | <https://ojs.uwindsor.ca/index.php/WYAJ/issue/archive> | Vol. 41 (2025) |

Not established: volume 1 year.

### jenvtllprac — Journal of Environmental Law and Practice

No page fetched raw (Berkeley and academia.cl catalogue hosts returned empty/block responses). Search snippet only says vol 1 no 1 Sept 1990, Carswell, ISSN 1181-7534. Registry examples show vol 11 in 2001, vol 14 in 2004 and 15 in 2004 and 2005 (two volumes in 2004/ vol 15 in 2004 and 2005) unchecked.

Not established: start (vol 1 no 1 Sept 1990) 1990.

### rdmcgill — Revue de droit de McGill

Duplicate entry: the same journal as mcgilllj; see that entry for start, current and anchors. The three anchors here are the registry examples (37/1992, 43/1998, 45/2000) found on the McGill issues page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| duplicate of mcgilllj | same journal | <https://lawjournal.mcgill.ca/article/prefece-to-the-twenty-fifth-anniversary-volume/> | known as well since 1977 as the Revue de Droit de McGill |
| vol. 37 | 1992 | <https://lawjournal.mcgill.ca/issues/> | Volume 37 1992 |
| vol. 43 | 1998 | <https://lawjournal.mcgill.ca/issues/> | Volume 43 1998 |
| vol. 45 | 2000 | <https://lawjournal.mcgill.ca/issues/> | Volume 45 2000 |
| title | Revue de droit de McGill (parallel title) 1977- | <https://lawjournal.mcgill.ca/article/prefece-to-the-twenty-fifth-anniversary-volume/> | known as well since 1977 as the Revue de Droit de McGill |

### ottawalrev — Ottawa Law Review

Faculty page states foundation in 1966 but not the volume 1 year, so start stays null. Anchors from Crossref (53/2021, 56/2024); Crossref has only 49 items for this title. Registry examples (1976,8),(1983,15),(1988,20),(1996-97,28) unchecked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| foundation | 1966 | <https://www.uottawa.ca/faculty-law/common-law/research/ottawa-law-review> | Since its foundation in 1966, the Ottawa Law Review |
| frequency | biannual | <https://www.uottawa.ca/faculty-law/common-law/research/ottawa-law-review> | published biannually |
| latest volume in Crossref | vol 56 (2024) | <https://api.crossref.org/journals/0048-2331/works?filter=from-pub-date:2024,until-pub-date:2024&rows=100&select=volume,issued,container-title> | {"volume":"56","issued":{"date-parts":[[2024]]},"container-title":["Ottawa Law Review"]} |
| vol. 53 | 2021 | <https://api.crossref.org/journals/0048-2331/works?filter=from-pub-date:2021,until-pub-date:2021&rows=100&select=volume,issued,container-title> | {"volume":"53","issued":{"date-parts":[[2021]]},"container-title":["Ottawa Law Review"]} |
| vol. 56 | 2024 | <https://api.crossref.org/journals/0048-2331/works?filter=from-pub-date:2024,until-pub-date:2024&rows=100&select=volume,issued,container-title> | {"volume":"56","issued":{"date-parts":[[2024]]},"container-title":["Ottawa Law Review"]} |

Not established: volume 1 year.

### michlrev — Michigan Law Review

Journal's own repository (repository.law.umich.edu) volume pages: each volume runs from autumn to the following spring/summer, so most volumes span two calendar years and consecutive volumes can share a year. Registry examples (1931) 29, (1938) 36, (1953) 52:1, (1974) 72, (1980) 79, (1986) 85 all consistent.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1902 | <https://repository.law.umich.edu/mlr/vol1/> | Volume 1 Issue 1 1902 |
| vol 2 spans 1903-1904 | Volume 2 Issue 1 1903 ... Issue 4 1904 | <https://repository.law.umich.edu/mlr/vol2/> | Volume 2 Issue 1 1903 Issue 2 1903 Issue 3 1903 Issue 4 1904 |
| vol 51 and vol 52 both in 1953 | vol 51 issue 3 1953; vol 52 issue 1 1953 | <https://repository.law.umich.edu/mlr/vol51/> | Volume 51 Issue 1 1952 Issue 2 1952 Issue 3 1953 |
| vol 52 spans 1953-1954 | vol 52 | <https://repository.law.umich.edu/mlr/vol52/> | Volume 52 Issue 1 1953 Issue 2 1953 Issue 3 1954 |
| vol 79 spans 1980-1981 | vol 79 | <https://repository.law.umich.edu/mlr/vol79/> | Volume 79 Issue 1 1980 Issue 2 1980 Issue 3 1981 |
| current | True | <https://repository.law.umich.edu/mlr/vol125/> | Volume 125 Issue 1 2026 Issue 2 2026 |
| vol. 1 | 1902 | <https://repository.law.umich.edu/mlr/vol1/> | Volume 1 Issue 1 1902 |
| vol. 29 | 1930 | <https://repository.law.umich.edu/mlr/vol29/> | Volume 29 Issue 1 1930 |
| vol. 52 | 1953 | <https://repository.law.umich.edu/mlr/vol52/> | Volume 52 Issue 1 1953 |
| vol. 72 | 1973 | <https://repository.law.umich.edu/mlr/vol72/> | Volume 72 Issue 1 1973 |
| vol. 85 | 1986 | <https://repository.law.umich.edu/mlr/vol85/> | Volume 85 Issue 1 1986 |
| vol. 125 | 2026 | <https://repository.law.umich.edu/mlr/vol125/> | Volume 125 Issue 1 2026 |
| title | Michigan Law Review 1902- | <https://repository.law.umich.edu/mlr/vol1/> | Volume 1 Issue 1 1902 |

### minnlrev — Minnesota Law Review

Crossref API only (scholarship.law.umn.edu blocked). Crossref has no items dated 1916, so vol 1 = 1917 is the earliest issued year there and may reflect deposit dating rather than the first issue. Registry examples (1918) 2:5, (1919) 3:5, (1924) 8, (1925) 9:4 agree with Crossref vols 2 and 9.

| Fact | Value | Source | Quote |
|---|---|---|---|
| vols 1 and 2 both in 1917 | vol 2 in 1917 | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:1917,until-pub-date:1917&rows=100&select=volume,issued,container-title> | {"volume":"2","issued":{"date-parts":[[1917]]},"container-title":["Minnesota Law Review"]} |
| current (vol 109 in 2025) | True | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:2025,until-pub-date:2025&rows=100&select=volume,issued,container-title> | {"volume":"109","issued":{"date-parts":[[2025]]},"container-title":["Minnesota Law Review"]} |
| vol. 1 | 1917 | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:1917,until-pub-date:1917&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1917]]},"container-title":["Minnesota Law Review"]} |
| vol. 2 | 1918 | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:1918,until-pub-date:1918&rows=100&select=volume,issued,container-title> | {"volume":"2","issued":{"date-parts":[[1918]]},"container-title":["Minnesota Law Review"]} |
| vol. 9 | 1925 | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:1925,until-pub-date:1925&rows=100&select=volume,issued,container-title> | {"volume":"9","issued":{"date-parts":[[1925]]},"container-title":["Minnesota Law Review"]} |
| vol. 74 | 1990 | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:1990,until-pub-date:1990&rows=100&select=volume,issued,container-title> | {"volume":"74","issued":{"date-parts":[[1990]]},"container-title":["Minnesota Law Review"]} |
| vol. 109 | 2025 | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:2025,until-pub-date:2025&rows=100&select=volume,issued,container-title> | {"volume":"109","issued":{"date-parts":[[2025]]},"container-title":["Minnesota Law Review"]} |
| title | Minnesota Law Review 1917- | <https://api.crossref.org/journals/0026-5535/works?filter=from-pub-date:1917,until-pub-date:1917&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1917]]},"container-title":["Minnesota Law Review"]} |

### iclq — International and Comparative Law Quarterly

Journal's own Cambridge Core all-issues page lists every volume with issue months. In the volumes shown (vols 45-75 and vol 1) each volume carries a single calendar year (issues Jan-Oct), so no volume spans two years and no year has two volumes there. Registry example years agree with the vol-to-year pattern shown (e.g. vol 53 = 2004 appears in the listing as 53.4 October 2004).

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1952 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Issue 1 January 1952 |
| current | True | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Volume 75 Issue 2 April 2026 |
| vol 1 issues Jan-Oct 1952 | one volume in 1952 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Issue 3 July 1952 |
| vol. 1 | 1952 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | 1952 - Volume 1 Archive content Issue 4 October 1952 |
| vol. 45 | 1996 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Volume 45 Issue 4 October 1996 |
| vol. 56 | 2007 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Volume 56 Issue 4 October 2007 |
| vol. 74 | 2025 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Volume 74 Issue 4 October 2025 |
| vol. 75 | 2026 | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | Volume 75 Issue 2 April 2026 |
| title | International and Comparative Law Quarterly 1952- | <https://www.cambridge.org/core/journals/international-and-comparative-law-quarterly/all-issues> | 1952 - Volume 1 Archive content Issue 4 October 1952 |

### canbuslj — Canadian Business Law Journal

Two journal-side pages state establishment in 1974 (managed from U of T Law). No page with volume/year pairs fetched; start (volume 1) stays null. Registry examples (1990) 16, (1992) 20, (2004) 40, (2004) 41, (2005) 41 cannot be checked; vol 41 appears under both 2004 and 2005, flagged for review. Page footer copyright 2026 is not proof of a recent issue, so current is only asserted from the site still describing "current editors".

| Fact | Value | Source | Quote |
|---|---|---|---|
| established | 1974 | <https://jackmanlaw.utoronto.ca/canadian-business-law-journal> | Established in 1974, the Canadian Business Law Journal |
| established (library guide) | 1974 | <https://learn.library.torontomu.ca/businesslaw/journals> | was established in 1974 to provide Canadian lawyers |

Not established: current; volume 1 year 1975.

### oscbull — Ontario Securities Commission Bulletin

Not a journal. Not researched further (regulator publication cited as [year] O.S.C. Bull.).

### rdub — Revue du Barreau

No raw page fetched. Search snippets suggest a title Revue du Barreau de la province de Quebec before 1969 and ISSN 0383-669X; unverified. Registry examples (1959) 19, (1980) 40, (1982) 42, (1991) 51, (1998) 58 unchecked.

Not established: earlier title Revue du Barreau de la province de Quebec to 1969 (catalogue snippet); start (vol 1 1941) 1941.

### valrev — Virginia Law Review

Crossref API only (virginialawreview.org blocked); Crossref has no Virginia items after 2004, so current is unproven. Registry example (1967) 53 and (1996) 82 agree.

| Fact | Value | Source | Quote |
|---|---|---|---|
| vol 1 and vol 2 both in 1914 | vol 2 in 1914 | <https://api.crossref.org/journals/0042-6601/works?filter=from-pub-date:1914,until-pub-date:1914&rows=100&select=volume,issued,container-title> | {"volume":"2","issued":{"date-parts":[[1914,10]]},"container-title":["Virginia Law Review"]} |
| vol. 1 | 1913 | <https://api.crossref.org/journals/0042-6601/works?filter=from-pub-date:1913,until-pub-date:1913&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1913,12]]},"container-title":["Virginia Law Review"]} |
| vol. 53 | 1967 | <https://api.crossref.org/journals/0042-6601/works?filter=from-pub-date:1967,until-pub-date:1967&rows=100&select=volume,issued,container-title> | {"volume":"53","issued":{"date-parts":[[1967,5]]},"container-title":["Virginia Law Review"]} |
| vol. 82 | 1996 | <https://api.crossref.org/journals/0042-6601/works?filter=from-pub-date:1996,until-pub-date:1996&rows=100&select=volume,issued,container-title> | {"volume":"82","issued":{"date-parts":[[1996,10]]},"container-title":["Virginia Law Review"]} |
| vol. 90 | 2004 | <https://api.crossref.org/journals/0042-6601/works?filter=from-pub-date:2004,until-pub-date:2004&rows=100&select=volume,issued,container-title> | {"volume":"90","issued":{"date-parts":[[2004,6]]},"container-title":["Virginia Law Review"]} |
| title | Virginia Law Review 1913- | <https://api.crossref.org/journals/0042-6601/works?filter=from-pub-date:1913,until-pub-date:1913&rows=100&select=volume,issued,container-title> | {"volume":"1","issued":{"date-parts":[[1913,12]]},"container-title":["Virginia Law Review"]} |

Not established: current.

### canjfaml — Canadian Journal of Family Law / Revue canadienne de droit familial

Two pages of the journal's own: site (cdnjfl.allard.ubc.ca, lists Vol. 1 No. 1 1978 to current) and open-access repository (commons.allard.ubc.ca; vols 26-36). They contradict on vol 31 no 2 (2018 repository vs 2019 site), and the site lists "Vol. 32, No. 2, 2022" under volume 34 (typo, repository says Number 2 (2022) for vol 34). Registry example (1998) 15:2 and (2004) 21:1, (2008) 24:1, (2009) 25:1 cannot be checked from fetched pages; repository gives 26 = 2010.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1978 | <https://cdnjfl.allard.ubc.ca/?p=2578> | Volume 1, Number 1, 1978 |
| current | True | <https://commons.allard.ubc.ca/can-j-fam-l/> | Current Issue: Volume 36, Number 2 (2025) |
| vol 28 spans 2012-2014 | Number 1 (2012), Number 2 (2014) | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 28 Number 1 (2012) |
| vol 29 spans 2014-2015 | Number 1 (2014), Number 2 (2015) | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 29 Number 1 (2014) Number 2 (2015) |
| vol 28 no 2 = 2014 and vol 29 no 1 = 2014 (two volumes in 2014) | 2014 | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 28 Number 1 (2012) |
| vol. 1 | 1978 | <https://cdnjfl.allard.ubc.ca/?p=2578> | Volume 1, Number 1, 1978 |
| vol. 26 | 2010 | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 26 Number 1 (2010) |
| vol. 30 | 2017 | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 30 Number 1 (2017) |
| vol. 33 | 2020 | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 33 Number 1 (2020) |
| vol. 36 | 2025 | <https://commons.allard.ubc.ca/can-j-fam-l/all_issues.html> | Volume 36 Number 1 (2025) |
| title | Canadian Journal of Family Law / Revue canadienne de droit familial 1978- | <https://cdnjfl.allard.ubc.ca/?p=2578> | Canadian Journal of Family Law/Revue canadienne de droit familial Volume 1, Number 1, 1978 |

Not established: vol 31 no 2 year (commons) 2018; vol 31 no 2 year (journal website) 2019.

### revconststud — Review of Constitutional Studies / Revue d'etudes constitutionnelles

Journal's own all-issues pages (5 pages). Issues are numbered volume.number (e.g. 19.1 = vol 19 no 1); volumes skip 7, 9, 10 in the listing (pages show 6.1, 8.1, 11.1). The latest issue listed is 27.2 (2023); current status unproven (web search said volume 28 moved online-only; not fetched). Registry example (2010) 19(1) conflicts: vol 19 no 1 is 2014; (2011) 15, 16 fit (15.2 = 2011; 16.1 = 2011); (1994) 2:1 and (2004) 9:1 - 2.1 = 1994 fits; vol 9 is not listed.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1993 | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/5/> | Volume 1.1 (1993) |
| volume 1 also 1994 | Volume 1.2 (1994) | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/5/> | Volume 1.2 (1994) |
| two volumes in 2019 (24.1/24.2 and 25.1) | Volume 24.2 (2019 - 2020); Volume 25.1 (2019 - 2020) | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/> | Volume 25.1 (2019 - 2020) |
| founding | 1993 | <https://constitutionalstudies.ca/publications/review> | Published by the Centre for Constitutional Studies since 1993 |
| latest listed | Volume 27.2 (2023) | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/> | Volume 27.2 (2023) |
| vol. 1 | 1993 | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/5/> | Volume 1.1 (1993) |
| vol. 5 | 1999 | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/4/> | Volume 5.1 (1999) |
| vol. 12 | 2006 | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/3/> | Volume 12.1 (2006) |
| vol. 19 | 2014 | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/2/> | Volume 19.1 (2014) |
| vol. 27 | 2023 | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/> | Volume 27.2 (2023) |
| title | Review of Constitutional Studies / Revue d'etudes constitutionnelles 1993- | <https://www.constitutionalstudies.ca/journals/review-of-constitutional-studies/all-issues/page/5/> | Volume 1.1 (1993) |

Not established: current; registry example 19(1) in 2010 Volume 19.1 is 2014.

### upalrev — University of Pennsylvania Law Review

Anchors are article-item dates from archive.org (JSTOR-derived records, raw JSON) and Crossref, not the journal pages. IA shows volumes straddling two calendar years (e.g. vol 57 = 1908 and 1909). Crossref earliest record is vol 93 (1945-3) and its container title is already University of Pennsylvania Law Review. Penn current-issue year is not shown on the fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1852 | <https://archive.law.upenn.edu/live/profiles/735-university-of-pennsylvania-law-review> | Founded in 1852 as the American Law Register, the University of Pennsylvania Law Review is the nation’s oldest law review. |
| volumes_span_years | volume 57 has items dated 1908 and 1909, volume 56 items dated 1908 (so 1908 holds parts of two volumes) | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Pennsylvania%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | (1908-10-01), page 52 |
| vol. 1 | 1852 | <https://archive.org/advancedsearch.php?q=title%3A%22American%20Law%20Register%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"jstor-3301951","title":"[untitled] The American Law Register (1852-1891) (1852-11-01), page 62","volume":"1","year":1852} |
| vol. 10 | 1862 | <https://archive.org/advancedsearch.php?q=title%3A%22American%20Law%20Register%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"jstor-3302871","title":"[untitled] The American Law Register (1852-1891), (1862-03-01), pages 318-319","volume":"10","year":1862} |
| vol. 56 | 1908 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Pennsylvania%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"jstor-3313425","title":"[untitled] University of Pennsylvania Law Review and American Law Register, (1908-04-01), pages 284-285","volume":"56","year":1908} |
| vol. 71 | 1922 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Pennsylvania%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"jstor-3314294","title":"[untitled] University of Pennsylvania Law Review and American Law Register, (1922-11-01), pages 95-96","volume":"71","year":1922} |
| vol. 93 | 1945 | <https://api.crossref.org/journals/0041-9907/works?rows=1&sort=issued&order=asc&filter=from-pub-date:1945-01-01,until-pub-date:1945-12-31&select=volume,issue,issued,container-title> | {"volume":"93","issue":"3","issued":{"date-parts":[[1945,3]]},"container-title":["University of Pennsylvania Law Review"]} |
| vol. 154 | 2006 | <https://api.crossref.org/journals/0041-9907/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2006-01-01,until-pub-date:2006-12-31&select=volume,issue,issued,container-title> | {"volume":"154","issue":"3","issued":{"date-parts":[[2006,1,1]]},"container-title":["University of Pennsylvania Law Review"]} |
| title | American Law Register 1852- | <https://archive.law.upenn.edu/live/profiles/735-university-of-pennsylvania-law-review> | Founded in 1852 as the American Law Register |
| title | University of Pennsylvania Law Review and American Law Register None- | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Pennsylvania%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | University of Pennsylvania Law Review and American Law Register (1908-04-01) |
| title | University of Pennsylvania Law Review None- | <https://en.wikipedia.org/wiki/University_of_Pennsylvania_Law_Review> | Former names American Law Register, American Law Register and Review, University of Pennsylvania Law Review and American Law Register |

Not established: current Penn Law Review site lists Volume 174 Issue 6 but no year on the fetched page; title_change_dates dates of the renames (1908? 1945?) not stated on any fetched page.

### manlj — Manitoba Law Journal

IA anchors are "Manitoba Law Journal 1968 - 1969: Vol 3 Index" etc. (scanned-serial records). Volume 3 = 1968-69 in IA, so the earlier "Vol. 3 = 2006" is the Underneath-the-Golden-Boy sub-series. Anchor for vol 36 uses 2012 (first issue); 2013 is the second issue.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1961 | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/about> | The Manitoba Law Journal (MLJ) is a peer-reviewed journal founded in 1961. |
| Vol.3 (2006) oddity | The OJS items "Vol. 3 No. (2006)", "Vol. 4 No. (2007)", "Vol. 5 No. (2008)", "Vol. 6 No. (2009)" are a separate series titled "Volume 3: Underneath the Golden Boy" etc., not the main run; the main run has Vol. 32 No. 1 (2006). So "Vol. 3 = 2006" is a series mix-up, not main-run evidence. | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive/3> | Volume 3: Underneath the Golden Boy Vol. 3 No. (2006) Volume 32 Issue 1 Vol. 32 No. 1 (2006) |
| two volumes in 2011 | Vol. 34 No. 3 and Vol. 35 No. 1 both 2011 | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive/3> | Vol. 35 No. 1 (2011) |
| volume spans two years | Vol. 46 No. 1 (2023) and Vol. 46 No. 2 (2024) | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive> | Vol. 46 No. 1 (2023) |
| volume spans two years (2) | Vol. 36 No. 1 (2012) and Vol. 36 No. 2 (2013) | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive/3> | Vol. 36 No. 2 (2013) |
| current | issue Vol. 47 No. 7 (2025) is the latest on the archive page | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive> | Vol. 47 No. 7 (2025) |
| vol. 3 | 1968 | <https://archive.org/advancedsearch.php?q=identifier%3Asim_manitoba-law-journal_%2A&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_manitoba-law-journal_1968-1969_3_index","title":"Manitoba Law Journal 1968 - 1969: Vol 3 Index","volume":"3","year":1968} |
| vol. 4 | 1970 | <https://archive.org/advancedsearch.php?q=identifier%3Asim_manitoba-law-journal_%2A&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_manitoba-law-journal_1970-1971_4_index","title":"Manitoba Law Journal 1970 - 1971: Vol 4 Index","volume":"4","year":1970} |
| vol. 7 | 1976 | <https://archive.org/advancedsearch.php?q=identifier%3Asim_manitoba-law-journal_%2A&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_manitoba-law-journal_1976-1977_7_cumulative-index","title":"Manitoba Law Journal 1976 - 1977: Vol 7 Index","volume":"7","year":1976} |
| vol. 30 | 2003 | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive/3> | Vol. 30 No. 1 (2003) |
| vol. 36 | 2012 | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive/3> | Vol. 36 No. 1 (2012) |
| vol. 41 | 2018 | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive/2> | Vol. 41 No. 1 (2018) |
| vol. 47 | 2025 | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/issue/archive> | Vol. 47 No. 7 (2025) |
| title | Manitoba Law Journal None- | <https://journals.library.ualberta.ca/themanitobalawjournal/index.php/mlj/about> | The Manitoba Law Journal (MLJ) is a peer-reviewed journal founded in 1961. |

Not established: earlier title "Manitoba Law School Journal" (1962-65) unknown.

### ajil — American Journal of International Law

Cambridge all-issues page lists each decade and the volume/year headings. No title change found. No year seen holding two volumes on the fetched page headings (each year heading names one volume).

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1907 | <https://api.crossref.org/journals/0002-9300/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[1907,1]]},"container-title":["American Journal of International\n                                                  Law"]} |
| published since 1907 | quarterly | <https://en.wikipedia.org/wiki/American_Journal_of_International_Law> | It has been published quarterly since 1907. |
| current | Volume 120 Issue 3 July 2026 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | Volume 120 Issue 3 July 2026 |
| vol. 1 | 1907 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | 1907 - Volume 1 |
| vol. 50 | 1956 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | 1956 - Volume 50 |
| vol. 52 | 1958 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | 1958 - Volume 52 |
| vol. 77 | 1983 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | 1983 - Volume 77 |
| vol. 100 | 2006 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | 2006 - Volume 100 |
| vol. 120 | 2026 | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | Volume 120 Issue 3 July 2026 |
| title | American Journal of International Law 1907- | <https://www.cambridge.org/core/journals/american-journal-of-international-law/all-issues> | 1907 - Volume 1 |

### goii — Gazette officielle du Québec, Partie 2

Not a journal; no fetch made.

Not established: nature government gazette.

### sasklrev — Saskatchewan Law Review

Only one verified (volume, year) pair; HeinOnline blocked. Early and middle anchors unverified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1936 | <https://sasklawreview.ca/history.php> | first appeared as a journal in 1936 and was called the Saskatchewan Bar Review |
| rename | 1967 | <https://sasklawreview.ca/history.php> | In 1967 the publication changed its name from the Saskatchewan Bar Review to the Saskatchewan Law Review |
| current | Volume 88(2), 2026 | <https://sasklawreview.ca/current-issue-content-outline.php> | Current issue Volume 88(2), 2026 |
| vol. 88 | 2026 | <https://sasklawreview.ca/current-issue-content-outline.php> | Current issue Volume 88(2), 2026 |
| title | Saskatchewan Bar Review 1936-1966 | <https://sasklawreview.ca/history.php> | first appeared as a journal in 1936 and was called the Saskatchewan Bar Review |
| title | Saskatchewan Law Review 1967- | <https://sasklawreview.ca/history.php> | In 1967 the publication changed its name from the Saskatchewan Bar Review to the Saskatchewan Law Review |

Not established: end of Bar Review = 1966 1966.

### uchicagolrev — University of Chicago Law Review

Chicago Unbound all-issues page (the journal's repository) gives one year per volume, but a volume spans parts of two calendar years: Vol. 4 is labelled 1936 while its issues 3-4 are cited 1937. Year labels here are the repository labels.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1933 | <https://lawreview.uchicago.edu/about> | The University of Chicago Law Review was first published in 1933, thirty-one years after the Law School began offering classes. |
| current | Volume 93 (2026) listed | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 93 (2026) |
| vol. 1 | 1933 | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 1 (1933) |
| vol. 10 | 1942 | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 10 (1942) |
| vol. 30 | 1962 | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 30 (1962) |
| vol. 56 | 1989 | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 56 (1989) |
| vol. 80 | 2013 | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 80 (2013) |
| vol. 93 | 2026 | <https://chicagounbound.uchicago.edu/uclrev/all_issues.html> | Volume 93 (2026) |
| title | University of Chicago Law Review 1933- | <https://lawreview.uchicago.edu/about> | The University of Chicago Law Review was first published in 1933 |

Not established: Vol. 4 issues 3 and 4 labelled 1936 by the journal repository registry examples cite these issues as 1937; the repository labels the whole of Vol. 4 "1936" (volume runs across two calendar years).

### rjt — Revue juridique Thémis de l’Université de Montréal

Start year not established: no fetched journal page states when Revue juridique Thémis began; catalogue records conflict. The Érudit page shows only vol 58 (2024). Registry examples (1967) 2 R.J.T. are unconfirmed.

| Fact | Value | Source | Quote |
|---|---|---|---|
| current | Volume 58, numéro 2, 2024 is the latest on Érudit (Érudit page: current and archived issues open access) | <https://www.erudit.org/fr/revues/themis/> | Volume 58, numéro 2, 2024 |
| vol. 58 | 2024 | <https://www.erudit.org/fr/revues/themis/> | Volume 58, numéro 2, 2024 |
| title | Thémis : revue des étudiants de la Faculté de droit de l’Université de Montréal (catalogue) None-1965 | <https://sclqld.org.au/catalogue/records/71805> | vol.1 (1951) - vol.15(1965) |

Not established: Thémis imprint vs holdings same record states "La Faculté, 1961-1965" and "vol.1 (1951) - vol.15(1965)"; catalogue imprint of the 1966 title 1966-1970; continues Thémis (Montréal, Québec : 1951); continued by Revue juridique Thémis; title Revue juridique Thémis de l’Université de Montréal 1966-1970.

### clelj — Canadian Labour and Employment Law Journal

Both anchors come from the single HeinOnline range sentence on an Osgoode library blog (2012); it does not show years per volume in between. Registry examples (1997) 4, (1998) 6, (2000) 8 are not confirmed. Current status unknown.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1992 | <https://libblog.osgoode.yorku.ca/2012/03/clelj-now-available-in-heinonline> | Volumes 1 -16 (1992 - 2011) |
| range covers 16 volumes over 1992-2011 | volumes are not annual | <https://libblog.osgoode.yorku.ca/2012/03/clelj-now-available-in-heinonline> | Volumes 1 -16 (1992 - 2011) |
| vol. 1 | 1992 | <https://libblog.osgoode.yorku.ca/2012/03/clelj-now-available-in-heinonline> | Volumes 1 -16 (1992 - 2011) |
| vol. 16 | 2011 | <https://libblog.osgoode.yorku.ca/2012/03/clelj-now-available-in-heinonline> | Volumes 1 -16 (1992 - 2011) |
| title | Canadian Labour and Employment Law Journal 1992- | <https://libblog.osgoode.yorku.ca/2012/03/clelj-now-available-in-heinonline> | The Canadian Labour and Employment Law Journal Volumes 1 -16 (1992 - 2011) |

Not established: registry start 1994 (Cardiff) 1994.

### cornelllrev — Cornell Law Review

Every volume spans two calendar years (Nov-start). Anchor "year" is the first year of the range given. Volume 90 is shown as (2004-2006) and Volume 103 as (2017-2019).

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1915 | <https://scholarship.law.cornell.edu/clr/vol1/iss1> | Volume 1, Issue 1 |
| title change to Cornell Law Review | 1967 | <https://en.wikipedia.org/wiki/Cornell_Law_Review> | In 1967, it committed itself to a bi-monthly publishing schedule and changed its name to the Cornell Law Review |
| vol. 1 | 1915 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 1 (1915-1916) |
| vol. 20 | 1934 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 20 (1934-1935) |
| vol. 40 | 1954 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 40 (1954-1955) |
| vol. 52 | 1966 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 52 (1966-1967) |
| vol. 53 | 1967 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 53 (1967-1968) |
| vol. 70 | 1984 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 70 (1984-1985) |
| vol. 90 | 2004 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 90 (2004-2006) |
| vol. 105 | 2020 | <https://scholarship.law.cornell.edu/clr/all_issues.html> | Volume 105 (2020-2021) |
| title | Cornell Law Quarterly 1915-1967 | <https://sclqld.org.au/catalogue/records/66106> | v.1 (1915) - v.52(1967) |
| title | Cornell Law Review 1967- | <https://en.wikipedia.org/wiki/Cornell_Law_Review> | In 1967, it committed itself to a bi-monthly publishing schedule and changed its name to the Cornell Law Review |

Not established: Wikipedia says Quarterly until 1966 Until 1966, the Cornell Law Review published four issues annually and was known as the Cornell Law Quarterly; current Cornell repository all-issues ends at Volume 105 (2020-2021); no fetched evidence of a later issue.

### callrev — California Law Review

IA records for vol 73 and 87 are the annual index records (1985, 1999); Crossref vol 94 = issued 2006-12-1. Volumes begin in November, so a volume covers two calendar years.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1912 | <https://www.californialawreview.org/about> | Founded in 1912, CLR was the first law review in the Western United States |
| vol 1 no 1 date | Nov. 1912 | <https://scholarly-societies.org/history/CalifLawRev.html> | Vol. 1, no. 1 (Nov. 1912)- |
| volumes straddle two calendar years | IA vol 3 has items dated 1914 and 1915; vol 4 1915 and 1916 | <https://archive.org/advancedsearch.php?q=title%3A%22California%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | "volume":"3","year":1914 |
| current | Volume 114 , August 2026 | <https://www.californialawreview.org/print> | Volume 114 , August 2026 |
| vol. 1 | 1912 | <https://api.crossref.org/journals/0008-1221/works?rows=1&sort=issued&order=asc&filter=from-pub-date:1912-01-01,until-pub-date:1912-12-31&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[1912,11]]},"container-title":["California Law Review"]} |
| vol. 5 | 1917 | <https://archive.org/advancedsearch.php?q=title%3A%22California%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"jstor-3474733","title":"[untitled] California Law Review, (1917-09-01), pages 508-509","volume":"5","year":1917} |
| vol. 73 | 1985 | <https://archive.org/advancedsearch.php?q=title%3A%22California%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_california-law-review_1985_73_index","title":"California Law Review 1985: Vol 73 Index","volume":"73","year":1985} |
| vol. 87 | 1999 | <https://archive.org/advancedsearch.php?q=title%3A%22California%20Law%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_california-law-review_1999_87_index","title":"California Law Review 1999: Vol 87 Index","volume":"87","year":1999} |
| vol. 94 | 2006 | <https://api.crossref.org/journals/0008-1221/works?rows=1&sort=issued&order=desc&filter=from-pub-date:2006-01-01,until-pub-date:2006-12-31&select=volume,issue,issued,container-title> | {"volume":"94","issue":"6","issued":{"date-parts":[[2006,12,1]]},"container-title":["California Law Review"]} |
| vol. 114 | 2026 | <https://www.californialawreview.org/print> | Volume 114 , August 2026 |
| title | California Law Review 1912- | <https://scholarly-societies.org/history/CalifLawRev.html> | California Law Review Vol. 1, no. 1 (Nov. 1912)- |

### science — Science

Anchors use the current (new-series, since 1895) numbering: vol 100 = Nov 1944 (IA), vol 260 = Apr-Jun 1993 (IA), then Crossref. Volume 1 appears three times (1880 old series, 1883, 1895 new series) so early volume numbers are ambiguous; the registry should key Science citations on volume >= 1 with series. science.org returned 403.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start (original series vol 1) | 1880 | <https://api.crossref.org/journals/0036-8075/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"os-1","issue":"1","issued":{"date-parts":[[1880,7,3]]},"container-title":["Science"]} |
| numbering restarts | v. 1 (1880)-v. 3 (1882); v. 1 (1883)-v. 23 (1894); new ser. v. 1 (1895)- | <https://scholarly-societies.org/history/Science.html> | v. 1 (1880)-v. 3 (1882); v. 1 (1883)-v. 23 (1894); new ser. v. 1 (1895)- |
| current | latest Crossref record vol 394 issue 6822 issued 2026-10-22 | <https://api.crossref.org/journals/0036-8075/works?rows=1&sort=issued&order=desc&select=volume,issue,issued,container-title> | {"volume":"394","issue":"6822","issued":{"date-parts":[[2026,10,22]]},"container-title":["Science"]} |
| four volumes per year | Crossref 2007 items: vols 315-318; 2010: 327-330; 2025: 387-390 | <https://api.crossref.org/journals/0036-8075/works?rows=60&filter=from-pub-date:2007-01-01,until-pub-date:2007-12-31&select=volume,issue,issued,container-title> | "volume":"318" |
| vol. 100 | 1944 | <https://archive.org/advancedsearch.php?q=identifier%3Asim_science_%2A&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_science_1944-07-28_100_2587","title":"Science 1944-07-28: Vol 100 Iss 2587","volume":"100","year":1944} |
| vol. 260 | 1993 | <https://archive.org/advancedsearch.php?q=identifier%3Asim_science_%2A&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_science_april-june-1993_260_index","title":"Science April-June 1993: Vol 260 Index","volume":"260","year":1993} |
| vol. 315 | 2007 | <https://api.crossref.org/journals/0036-8075/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2007-01-01,until-pub-date:2007-12-31&select=volume,issue,issued,container-title> | {"volume":"315","issue":"5808","issued":{"date-parts":[[2007,1,5]]},"container-title":["Science"]} |
| vol. 327 | 2010 | <https://api.crossref.org/journals/0036-8075/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2010-01-01,until-pub-date:2010-12-31&select=volume,issue,issued,container-title> | {"volume":"327","issue":"5961","issued":{"date-parts":[[2010,1]]},"container-title":["Science"]} |
| vol. 387 | 2025 | <https://api.crossref.org/journals/0036-8075/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2025-01-01,until-pub-date:2025-12-31&select=volume,issue,issued,container-title> | {"volume":"387","issue":"6729","issued":{"date-parts":[[2025,1,3]]},"container-title":["Science"]} |
| vol. 390 | 2025 | <https://api.crossref.org/journals/0036-8075/works?rows=1&sort=issued&order=desc&filter=from-pub-date:2025-01-01,until-pub-date:2025-12-31&select=volume,issue,issued,container-title> | {"volume":"390","issue":"6779","issued":{"date-parts":[[2025,12,18]]},"container-title":["Science"]} |
| title | Science 1880- | <https://scholarly-societies.org/history/Science.html> | Science v. 1 (1880)-v. 3 (1882); v. 1 (1883)-v. 23 (1894); new ser. v. 1 (1895)- |

### constforum — Constitutional Forum constitutionnel

Start left null because the page says first published 1989 as a newsletter and became a journal in the early 1990s; no page gives the Vol. 1 year. OJS archive only reaches 2011 (Vol. 20).

| Fact | Value | Source | Quote |
|---|---|---|---|
| first published (as newsletter) | 1989 | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/about> | was first published in 1989 as a quarterly newsletter |
| two volumes in 2013 | Vol. 21 No. 2 (2013) and Vol. 22 No. 1-3 (2013) | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/issue/archive/2> | Vol. 21 No. 2 (2013) |
| current | Vol. 35 No. 1 (2026) | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/issue/archive> | Vol. 35 No. 1 (2026) |
| vol. 20 | 2011 | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/issue/archive/2> | Vol. 20 No. 1 (2011) |
| vol. 24 | 2015 | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/issue/archive/2> | Vol. 24 No. 1 (2015) |
| vol. 28 | 2019 | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/issue/archive/2> | Vol. 28 No. 1 (2019) |
| vol. 35 | 2026 | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/issue/archive> | Vol. 35 No. 1 (2026) |
| title | Constitutional Forum constitutionnel 1989- | <https://journals.library.ualberta.ca/constitutional_forum/index.php/constitutional_forum/about> | Constitutional Forum constitutionnel was first published in 1989 as a quarterly newsletter |

### nyulrev — New York University Law Review

Issues page lists "YEAR Volume N Number 6, December ... Number 1, April": each volume runs April to December within the labelled year. Only 1999-2026 anchors available on the journal site; earlier volumes not verified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1924 | <https://www.law.nyu.edu/news/nyu-law-review-100-years> | just over a century after the first issue debuted in April 1924 |
| name since 1950 | 1950 | <https://en.wikipedia.org/wiki/New_York_University_Law_Review> | before obtaining its ongoing name in 1950 |
| second volume 1925 | 1925 | <https://www.law.nyu.edu/news/nyu-law-review-100-years> | became the first woman editor-in-chief with the Law Review ’s second volume |
| current | Volume 101 Number 3, June 2026 | <https://nyulawreview.org/issues/> | 2026 Volume 101 Number 3, June |
| vol. 74 | 1999 | <https://nyulawreview.org/issues/> | 1999 Volume 74 Number 6, December |
| vol. 75 | 2000 | <https://nyulawreview.org/issues/> | 2000 Volume 75 Number 6, December |
| vol. 101 | 2026 | <https://nyulawreview.org/issues/> | 2026 Volume 101 Number 3, June |
| title | Annual Review of the Law School of New York University 1924- | <https://nyulawreview.org/about/> | Founded in 1924 as the Annual Review of the Law School of New York University |
| title | New York University Law Quarterly Review None- | <https://en.wikipedia.org/wiki/New_York_University_Law_Review> | New York University Law Quarterly Review |
| title | New York University Law Review 1950- | <https://en.wikipedia.org/wiki/New_York_University_Law_Review> | before obtaining its ongoing name in 1950 |

### queqb — Quebec Queen's Bench Reports

Not a journal; no fetch made.

### geolj — The Georgetown Law Journal

Library archive volume labels are academic-year ranges; "year" here is the first year of the range. Registry examples (2000) 89, (2003) 91, (2007) 96 not directly checked.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1912 | <https://www.law.georgetown.edu/georgetown-law-journal/about/> | Volume 1, Issue 1 was published in November 1912, under the supervision of Editor-in-Chief Eugene Quay. |
| volume spans two years | each volume is labelled with two years on the Georgetown Law Library archive, e.g. v.1 (1912-1913) | <https://aspace.ll.georgetown.edu/public/repositories/4/resources/268/digitized?page=2> | Georgetown Law Journal, v.1 (1912-1913) |
| current | Volume 114 Issue 6 Date Jun. 2026 | <https://www.law.georgetown.edu/georgetown-law-journal/> | Volume 114 Issue 6 Date Jun. 2026 |
| vol. 1 | 1912 | <https://aspace.ll.georgetown.edu/public/repositories/4/resources/268/digitized?page=2> | Georgetown Law Journal, v.1 (1912-1913) |
| vol. 17 | 1928 | <https://aspace.ll.georgetown.edu/public/repositories/4/resources/268/digitized?page=3> | Georgetown Law Journal, v.17 (1928-1929) |
| vol. 38 | 1949 | <https://aspace.ll.georgetown.edu/public/repositories/4/resources/268/digitized?page=5> | Georgetown Law Journal, v.38 (1949-1950) |
| vol. 49 | 1960 | <https://aspace.ll.georgetown.edu/public/repositories/4/resources/268/digitized?page=7> | Georgetown Law Journal, v.49 (1960-1961) |
| vol. 73 | 1984 | <https://aspace.ll.georgetown.edu/public/repositories/4/resources/268/digitized?page=9> | Georgetown Law Journal, v.73 (1984-1985) |
| vol. 114 | 2026 | <https://www.law.georgetown.edu/georgetown-law-journal/> | Volume 114 Issue 6 Date Jun. 2026 |
| title | The Georgetown Law Journal 1912- | <https://www.law.georgetown.edu/georgetown-law-journal/about/> | Volume 1, Issue 1 was published in November 1912 |

### utfaclrev — University of Toronto Faculty of Law Review

No fetched page gives any volume-year pair. Start left null (page says first published in 1942, but the page does not call it Vol. 1). Current status not determined.

| Fact | Value | Source | Quote |
|---|---|---|---|
| first published | 1942 | <https://utflr.ca/about> | First published in 1942 under the name School of Law Review |
| title | School of Law Review 1942- | <https://utflr.ca/about> | First published in 1942 under the name School of Law Review |

Not established: Vol. 1 (1942) on HeinOnline U of T library says full run from Vol. 1 (1942).

### rsq — Revised Statutes of Quebec

Not a journal; no fetch made.

### lqr — The Law Quarterly Review

IA scanned-serial records (title "The Law Quarterly Review 1885: Vol 1 Index") give one volume per calendar year 1885-1900 and 1989-2002. Crossref does not carry the journal. Current status unverified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| vol. 1 | 1885 | <https://archive.org/advancedsearch.php?q=title%3A%22Law%20Quarterly%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_law-quarterly-review_1885_1_index","title":"The Law Quarterly Review 1885: Vol 1 Index","volume":"1","year":1885} |
| vol. 10 | 1894 | <https://archive.org/advancedsearch.php?q=title%3A%22Law%20Quarterly%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_law-quarterly-review_1894-01_10_37","title":"The Law Quarterly Review 1894-01: Vol 10 Iss 37","volume":"10","year":1894} |
| vol. 16 | 1900 | <https://archive.org/advancedsearch.php?q=title%3A%22Law%20Quarterly%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_law-quarterly-review_1900-07_16_63","title":"The Law Quarterly Review 1900-07: Vol 16 Iss 63","volume":"16","year":1900} |
| vol. 105 | 1989 | <https://archive.org/advancedsearch.php?q=title%3A%22Law%20Quarterly%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_law-quarterly-review_1989_105_index","title":"The Law Quarterly Review 1989: Vol 105 Index","volume":"105","year":1989} |
| vol. 118 | 2002 | <https://archive.org/advancedsearch.php?q=title%3A%22Law%20Quarterly%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_law-quarterly-review_2002_118_index","title":"The Law Quarterly Review 2002: Vol 118 Index","volume":"118","year":2002} |
| title | The Law Quarterly Review 1885- | <https://archive.org/advancedsearch.php?q=title%3A%22Law%20Quarterly%20Review%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | The Law Quarterly Review 1885: Vol 1 Index |

Not established: current no fetched issue after 2002 (IA) found.

### stetsonlrev — Stetson Law Review

Nothing verified: stetson.edu pages return 403 and no archive/Crossref page was reachable.

Not established: origin Stetson Intramural Law Review 1970-1978 (search snippet only).

### cjwl — Canadian Journal of Women and the Law / Revue Femmes et Droit

Crossref has records only from 2009 (vol 21), so there are no anchors before then; start left null (Wikipedia secondary).

| Fact | Value | Source | Quote |
|---|---|---|---|
| established | 1985 | <https://en.wikipedia.org/wiki/Canadian_Journal_of_Women_and_the_Law> | It was established in 1985 and is published by the University of Toronto Press |
| Crossref earliest record | vol 21 issued 2009-1 | <https://api.crossref.org/journals/0832-8781/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"21","issue":"1","issued":{"date-parts":[[2009,1]]},"container-title":["Canadian Journal of Women and the Law"]} |
| current | vol 36 iss 2 issued 2025-7-1 | <https://api.crossref.org/journals/0832-8781/works?rows=1&sort=issued&order=desc&select=volume,issue,issued,container-title> | {"volume":"36","issue":"2","issued":{"date-parts":[[2025,7,1]]},"container-title":["Canadian Journal of Women and the Law"]} |
| vol. 21 | 2009 | <https://api.crossref.org/journals/0832-8781/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2009-01-01,until-pub-date:2009-12-31&select=volume,issue,issued,container-title> | {"volume":"21","issue":"1","issued":{"date-parts":[[2009,1]]},"container-title":["Canadian Journal of Women and the Law"]} |
| vol. 27 | 2015 | <https://api.crossref.org/journals/0832-8781/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2015-01-01,until-pub-date:2015-12-31&select=volume,issue,issued,container-title> | {"volume":"27","issue":"1","issued":{"date-parts":[[2015,4]]},"container-title":["Canadian Journal of Women and the Law"]} |
| vol. 36 | 2025 | <https://api.crossref.org/journals/0832-8781/works?rows=1&sort=issued&order=desc&filter=from-pub-date:2025-01-01,until-pub-date:2025-12-31&select=volume,issue,issued,container-title> | {"volume":"36","issue":"2","issued":{"date-parts":[[2025,7,1]]},"container-title":["Canadian Journal of Women and the Law"]} |
| title | Canadian Journal of Women and the Law 1985- | <https://en.wikipedia.org/wiki/Canadian_Journal_of_Women_and_the_Law> | It was established in 1985 and is published by the University of Toronto Press |

### amjbioeth — The American Journal of Bioethics

Container title in Crossref is "American Journal of Bioethics" in 2001 and "The American Journal of Bioethics" on 2026 records; the date of that change was not found. Registry examples with "8 :9" garble are mojibake.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Vol. 1 issue 1 first record | 2001 | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[2001,1,1]]},"container-title":["American Journal of Bioethics"]} |
| established (Wikipedia) | 1999 | <https://en.wikipedia.org/wiki/American_Journal_of_Bioethics> | The journal was established in 1999 by founding editor-in-chief Glenn McGee and David Magnus. |
| current | articles issued 2026-10-23 | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=desc&select=volume,issue,issued,container-title> | {"issued":{"date-parts":[[2026,10,23]]},"container-title":["The American Journal of Bioethics"]} |
| vol. 1 | 2001 | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2001-01-01,until-pub-date:2001-12-31&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[2001,1,1]]},"container-title":["American Journal of Bioethics"]} |
| vol. 8 | 2008 | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2008-01-01,until-pub-date:2008-12-31&select=volume,issue,issued,container-title> | {"volume":"8","issue":"1","issued":{"date-parts":[[2008,1,25]]},"container-title":["The American Journal of Bioethics"]} |
| vol. 10 | 2010 | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2010-01-01,until-pub-date:2010-12-31&select=volume,issue,issued,container-title> | {"volume":"10","issue":"1","issued":{"date-parts":[[2010,1,8]]},"container-title":["The American Journal of Bioethics"]} |
| vol. 24 | 2024 | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2024-01-01,until-pub-date:2024-12-31&select=volume,issue,issued,container-title> | {"volume":"24","issue":"1","issued":{"date-parts":[[2024,1,2]]},"container-title":["The American Journal of Bioethics"]} |
| title | American Journal of Bioethics 2001- | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[2001,1,1]]},"container-title":["American Journal of Bioethics"]} |
| title | The American Journal of Bioethics None- | <https://api.crossref.org/journals/1526-5161/works?rows=1&sort=issued&order=desc&select=volume,issue,issued,container-title> | {"issued":{"date-parts":[[2026,10,23]]},"container-title":["The American Journal of Bioethics"]} |

Not established: publisher dates (MIT Press 2001, Taylor & Francis 2008) unknown.

### jlsocpoly — Journal of Law and Social Policy

Osgoode Digital Commons labels each volume with a single year; several volumes share a year (3 and 4 in 1988; 28-30 in 2018; 34 and 35 in 2021).

| Fact | Value | Source | Quote |
|---|---|---|---|
| Vol. 3 and Vol. 4 both 1988 |  | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 4 (1988) |
| Vol. 28, 29, 30 all 2018 |  | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 28 (2018) |
| current | Volume 38 (2026) | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 38 (2026) |
| vol. 1 | 1985 | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 1 (1985) |
| vol. 3 | 1988 | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 3 (1988) |
| vol. 10 | 1994 | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 10 (1994) |
| vol. 15 | 2000 | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 15 (2000) |
| vol. 22 | 2009 | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 22 (2009) |
| vol. 38 | 2026 | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 38 (2026) |
| title | Journal of Law and Social Policy 1985- | <https://digitalcommons.osgoode.yorku.ca/jlsp/all_issues.html> | Volume 1 (1985) |

### iowalrev — Iowa Law Review

Journal archive page covers only volume 99 onwards. Volume of the Review in 1925 (vol 11?) not shown on any fetched page. Start left null since the journal-own page does not give it; catalogue and Wikipedia say 1915. Volumes are labelled with academic-year ranges.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start (Iowa Law Bulletin v.1) | 1915 | <https://sclqld.org.au/catalogue/records/66591> | v.1 (1915) - v.10(1925) |
| rename 1925 | 1925 | <https://en.wikipedia.org/wiki/Iowa_Law_Review> | It changed its name to Iowa Law Review in 1925 |
| current | Volume 111, Issue 5 July 2025 | <https://ilr.law.uiowa.edu/volume-111-issue-5> | Volume 111, Issue 5 July 2025 |
| vol. 1 | 1915 | <https://sclqld.org.au/catalogue/records/66591> | v.1 (1915) - v.10(1925) |
| vol. 10 | 1925 | <https://sclqld.org.au/catalogue/records/66591> | v.1 (1915) - v.10(1925) |
| vol. 99 | 2013 | <https://ilr.law.uiowa.edu/print-edition-archive> | Volume 99 2013 –2014 |
| vol. 100 | 2014 | <https://ilr.law.uiowa.edu/print-edition-archive> | Volume 100 Centennial Edition 2014 –2015 |
| vol. 111 | 2025 | <https://ilr.law.uiowa.edu/print-edition-archive> | Volume 111 2025–2026 |
| title | Iowa Law Bulletin 1915-1925 | <https://sclqld.org.au/catalogue/records/66591> | v.1 (1915) - v.10(1925) |
| title | Iowa Law Review 1925- | <https://en.wikipedia.org/wiki/Iowa_Law_Review> | It changed its name to Iowa Law Review in 1925 |

### jlecon — The Journal of Law and Economics

One volume per calendar year on the repository page from Vol. 1 (1958) onward in the sampled anchors (annual numbering); no year shows two volumes in the sampled range.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Crossref vol 1 issue | issued 1958-10 | <https://api.crossref.org/journals/0022-2186/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[1958,1]]},"container-title":["The Journal of Law and Economics"]} |
| current | Volume 69 Number 1 (2026) | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 69 Number 1 (2026) |
| vol. 1 | 1958 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 1 Number 1 (1958) |
| vol. 10 | 1967 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 10 Number 1 (1967) |
| vol. 20 | 1977 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 20 Number 1 (1977) |
| vol. 30 | 1987 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 30 Number 1 (1987) |
| vol. 40 | 1997 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 40 Number 1 (1997) |
| vol. 50 | 2007 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 50 Number 1 (2007) |
| vol. 60 | 2017 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 60 Number 1 (2017) |
| vol. 69 | 2026 | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 69 Number 1 (2026) |
| title | The Journal of Law and Economics 1958- | <https://chicagounbound.uchicago.edu/jle/all_issues.html> | Volume 1 Number 1 (1958) |

### bcstudies — BC Studies: The British Columbian Quarterly

BC Studies is numbered by issue, not volume: the "volume" field here is the issue number (registry cites "(1992) 95 BC Studies"). Issue 1 start left null because the journal's own archive (Winter 1968/69) and About page (spring 1969) disagree. Anchor year for No. 1 is 1968 (first year in "1968/69"). Issues straddle winter (e.g. Winter 1993/94 No. 100).

| Fact | Value | Source | Quote |
|---|---|---|---|
| current | Summer No. 230 (2026) | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive> | Summer No. 230 (2026) |
| vol. 1 | 1968 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/9> | Winter 1968/69 No. 1 |
| vol. 23 | 1974 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/8> | Autumn 1974 No. 23 |
| vol. 74 | 1987 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/6> | Summer 1987 No. 74 |
| vol. 99 | 1993 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/6> | Autumn 1993 No. 99 |
| vol. 127 | 2000 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/5> | Autumn 2000 No. 127 |
| vol. 155 | 2007 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/3> | Autumn 2007 No. 155 |
| vol. 190 | 2016 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive/2> | Summer 2016 No. 190 |
| vol. 230 | 2026 | <https://ojs.library.ubc.ca/index.php/bcstudies/issue/archive> | Summer No. 230 (2026) |
| title | BC Studies: The British Columbian Quarterly 1969- | <https://ojs.library.ubc.ca/index.php/bcstudies/about> | First published in the spring of 1969 |

Not established: start: About page says first published spring 1969 1969; start: archive lists issue No. 1 as Winter 1968/69 1968/69.

### austljfaml — Australian Journal of Family Law

No volume-year pair fetched. Catalogue imprint 1986- only.

| Fact | Value | Source | Quote |
|---|---|---|---|
| catalogue imprint | Sydney : Butterworths, 1986- | <https://sclqld.org.au/catalogue/records/20071> | Sydney : Butterworths, 1986- |
| title | Australian Journal of Family Law 1986- | <https://sclqld.org.au/catalogue/records/20071> | Sydney : Butterworths, 1986- |

### cancrimlrev — Canadian Criminal Law Review

No volume-year pair fetched.

| Fact | Value | Source | Quote |
|---|---|---|---|
| catalogue imprint | Scarborough, Ont. : Carswell, 1996- | <https://sclqld.org.au/catalogue/records/62989> | Scarborough, Ont. : Carswell, 1996- |
| title | Canadian Criminal Law Review 1996- | <https://sclqld.org.au/catalogue/records/62989> | Scarborough, Ont. : Carswell, 1996- |

### cded — Les Cahiers de droit

Anchors are Érudit issue URL slugs (year-vNN-nN) on the journal's Érudit page. Érudit raw fetch worked for the cd1 revue page. Vol 2 anchor uses the first issue slug (1955-v2-n1).

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1954 | <https://www.erudit.org/fr/revues/cd1/> | Fondée en 1954 à la Faculté de droit de l’Université Laval |
| Crossref vol 1 issue 1 | issued 1954 | <https://api.crossref.org/journals/0007-974X/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[1954]]},"container-title":["Les Cahiers de droit"]} |
| current | Volume 67, numéro 3, septembre 2026 | <https://www.erudit.org/fr/revues/cd1/> | Volume 67, numéro 3, septembre 2026 |
| title | Les Cahiers de droit 1954- | <https://www.erudit.org/fr/revues/cd1/> | Fondée en 1954 à la Faculté de droit de l’Université Laval |

Not established: two volumes in 1955 v1 n2 and v2 n1 both 1955; two volumes in 1965 v6 n2 and v7 n1-n3 both 1965; vol. 1 1954; vol. 15 1974; vol. 2 1955; vol. 26 1985; vol. 41 2000; vol. 67 2026; volume spans two years v9: 1967 n1, n2 and 1968 n3.

### utorontolj — University of Toronto Law Journal

IA labels: vol 1 = "1935 - 1936", vol 2 = 1937/1938. The "The" in the title is dropped by IA after 1953 ("University of Toronto Law Journal 1956") and by Crossref later records; exact change year not shown.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Crossref vol 1 issue 1 | issued 1935 | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[1935]]},"container-title":["The University of Toronto Law Journal"]} |
| established 1935 | 1935 | <https://en.wikipedia.org/wiki/University_of_Toronto_Law_Journal> | It was established in 1935 by William Paul McClure Kennedy |
| current | vol 76 iss 3 issued 2026-7-1 | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=desc&select=volume,issue,issued,container-title> | {"volume":"76","issue":"3","issued":{"date-parts":[[2026,7,1]]},"container-title":["University of Toronto Law Journal"]} |
| vol. 1 | 1935 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Toronto%20Law%20Journal%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_university-of-toronto-law-journal_1935-1936_1_index","title":"The University of Toronto Law Journal 1935 - 1936: Vol 1 Index","volume":"1","year":1935} |
| vol. 10 | 1953 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Toronto%20Law%20Journal%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_university-of-toronto-law-journal_1953-1954_10_index","title":"The University of Toronto Law Journal 1953 - 1954: Vol 10 Index","volume":"10","year":1953} |
| vol. 20 | 1970 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Toronto%20Law%20Journal%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_university-of-toronto-law-journal_1970_20_index","title":"University of Toronto Law Journal 1970: Vol 20 Index","volume":"20","year":1970} |
| vol. 35 | 1985 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Toronto%20Law%20Journal%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_university-of-toronto-law-journal_1985_35_contents","title":"University of Toronto Law Journal 1985: Vol 35 Table of Contents","volume":"35","year":1985} |
| vol. 40 | 1990 | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=asc&filter=from-pub-date:1990-01-01,until-pub-date:1990-12-31&select=volume,issue,issued,container-title> | {"volume":"40","issue":"3","issued":{"date-parts":[[1990]]},"container-title":["The University of Toronto Law Journal"]} |
| vol. 46 | 1996 | <https://archive.org/advancedsearch.php?q=title%3A%22University%20of%20Toronto%20Law%20Journal%22%20AND%20mediatype%3Atexts&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=volume&fl%5B%5D=year&rows=5000&output=json> | {"identifier":"sim_university-of-toronto-law-journal_1996_46_contents","title":"University of Toronto Law Journal 1996: Vol 46 Table of Contents","volume":"46","year":1996} |
| vol. 65 | 2015 | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2015-01-01,until-pub-date:2015-12-31&select=volume,issue,issued,container-title> | {"volume":"65","issue":"1","issued":{"date-parts":[[2015,1]]},"container-title":["University of Toronto Law Journal"]} |
| vol. 75 | 2025 | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=asc&filter=from-pub-date:2025-01-01,until-pub-date:2025-12-31&select=volume,issue,issued,container-title> | {"volume":"75","issue":"1","issued":{"date-parts":[[2025,1,1]]},"container-title":["University of Toronto Law Journal"]} |
| vol. 76 | 2026 | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=desc&filter=from-pub-date:2026-01-01,until-pub-date:2026-12-31&select=volume,issue,issued,container-title> | {"volume":"76","issue":"3","issued":{"date-parts":[[2026,7,1]]},"container-title":["University of Toronto Law Journal"]} |
| title | The University of Toronto Law Journal 1935- | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=asc&select=volume,issue,issued,container-title> | {"volume":"1","issue":"1","issued":{"date-parts":[[1935]]},"container-title":["The University of Toronto Law Journal"]} |
| title | University of Toronto Law Journal None- | <https://api.crossref.org/journals/0042-0220/works?rows=1&sort=issued&order=desc&select=volume,issue,issued,container-title> | {"volume":"76","issue":"3","issued":{"date-parts":[[2026,7,1]]},"container-title":["University of Toronto Law Journal"]} |

### canjadminlprac — Canadian Journal of Administrative Law & Practice

No volume-year pair fetched.

| Fact | Value | Source | Quote |
|---|---|---|---|
| catalogue imprint | Carswell Legal Publications, 1987- | <https://sclqld.org.au/catalogue/records/62937> | Carswell Legal Publications, 1987- |
| title | Canadian Journal of Administrative Law & Practice 1987- | <https://sclqld.org.au/catalogue/records/62937> | Carswell Legal Publications, 1987- |

## Reporter editions

### SCR (scr)

Registry start 1876 agrees with two catalogues and HeinOnline. 1876 is the first volume's decision year; printed imprint is 1878. Open-ended Bodleian holdings line is not proof of a recent volume.

| Fact | Value | Source | Quote |
|---|---|---|---|
| first volume year | 1876 | <https://sclqld.org.au/catalogue/records/19854> | Publication details: Ottawa : Queen's Pr, 1876-. |
| first volume year (publisher-type source) | 1876 | <https://home.heinonline.org/?p=3763> | beginning with the first volume published in 1876 |
| title changes: Reports of the SCC to v.64; Canada Law Reports 1923-1969; Canada Supreme Court Reports 1970- | 1923; 1970 | <https://sclqld.org.au/catalogue/records/19854> | Note: 1878-1923, Reports of the Supreme Court of Canada-- 1923-1969, Canada Law Reports : Supreme Court of Canada. |
| v.64 is the last numbered volume; later volumes identified by year | 1923 | <https://search.library.wisc.edu/catalog/999890332402121> | Publication Date Vol. 1 (1876/1877)-v. 64 (1922/1923) Physical Details 64 volumes |
| vol. 1 | 1876 | <https://search.library.wisc.edu/catalog/999890332402121> | Vol. 1 (1876/1877)-v. 64 (1922/1923) |
| vol. 64 | 1923 | <https://lib.unb.ca/eresources?id=3166> | for citations to volumes 1 (1876/77)-64 (1923) |
| numbering | volume 1876-1923 | <https://lib.unb.ca/eresources?id=3166> | for citations to volumes 1 (1876/77)-64 (1923), but enter the volume year without brackets and the volume number if applicable for citations to volumes published after v.64 (1923) |
| numbering | year_and_volume 1923- | <https://lib.unb.ca/eresources?id=3166> | but enter the volume year without brackets and the volume number if applicable for citations to volumes published after v.64 (1923) |

Not established: Melbourne guide start year of first series 1875; current (recent volume proof) not fetched; imprint years of vols 1-64 (differs from 1876) 1878-1923; imprint years of vols 1-64 (library catalogue) 1878-1923.

### AC (ac)

AC from 1891 is dated annual volumes. Pre-1891 'A.C.' cites are Law Reports Appeal Cases 1875-1890 (App Cas), a separate volume-numbered series; the sclqld record shows vol1 (1875) - vol5(1890) which looks like a catalogue shorthand, volume counts not verified. 1891-onwards is Appeal Cases within The Law Reports.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start of Appeal Cases (AC) series | 1891 | <https://www.sclqld.org.au/catalogue/records/20130> | Publication details: London : Incorporated Council of Law Reporting, 1891-. |
| continues today (ICLR present tense) | true | <https://www.iclr.co.uk/products/law-report-series/the-law-reports/> | Published monthly since 1865, The Law Reports are widely regarded as the most authoritative series of law reports for England and Wales. |
| sub-volume numbers within a year | year in brackets; volume number used in some years | <https://www.iclr.co.uk/products/law-report-series/the-law-reports/> | In some earlier years, individual subdivisions may also appear with a volume number. |
| predecessor series Appeal Cases (House of Lords) 1875-1890 cited as App Cas / LR App Cas, numbered by volume | 1875-1890 | <https://www.sclqld.org.au/catalogue/records/20124> | The law reports : Appeal cases, 1875-1890. Copy direct link Share Available online Online via ICLR online vol1 (1875) - vol5(1890) |
| Registry example (1888) 14 A.C. predates 1891 | cannot be this series | <https://www.sclqld.org.au/catalogue/records/20124> | Publication details: London : Inc Council of Law Reporting, 1876-1890. |
| numbering | year 1891- | <https://www.iclr.co.uk/history/> | In 1891 the Council introduced the simplified arrangement of dated annual volumes in four series which continues to this day, comprising Appeal Cases |

### DLR (4th) (dlr)

Current=true rests on a UK retailer's subscription release list (Carswell, Dominion Law Reports 4th Series) with a release dated Oct 2026; no publisher page fetched. No DLR (5th) found in any fetched page (registry editions list a 5th; unverified either way). Anchor 274/2007 is from a library's print holdings statement.

| Fact | Value | Source | Quote |
|---|---|---|---|
| 4th series first volume | 1984 | <https://search.library.wisc.edu/catalog/999618282902121> | Publication Toronto : Canada Law Book Co., 1912- Publication Date Vol. 1 (1912)-v. 70 (1922); 1923, v. 1-1955, v. 5; 2nd ser., v. 1 (1956)-v. 70 (1968); 3rd ser., v. 1 (1969)-v. 150 (1984); 4th ser., v. 1 (1984)- |
| 3rd series ends 1984 (range) | 1969-1984 | <https://www.courthouselibrary.ca/catalogue-item?cid=8970> | Publication Date: 1984 Publication Date Range: 1969-1984. |
| still issuing: retailer lists monthly releases through October 2026 | current | <https://www.wildy.com/isbn/0012-5350BV/dominion-law-reports-4th-series-bound-volumes-only-the-carswell-company-ltd> | Latest Release: October 12, 2026 Publisher: The Carswell Company Ltd |
| vol. 1 | 1984 | <https://search.library.wisc.edu/catalog/999618282902121> | 4th ser., v. 1 (1984)- |
| vol. 274 | 2007 | <https://sclqld.org.au/catalogue/records/19861> | 4th series, vol. 1 (1984) - vol. 274 (2007) |
| numbering | volume 1984- | <https://search.library.wisc.edu/catalog/999618282902121> | 3rd ser., v. 1 (1969)-v. 150 (1984); 4th ser., v. 1 (1984)- |

Not established: holdings-only range (not the series run) 1912-2009.

### CCC (3d) (ccc)

Volume 1 of the 3d series is dated 1983 by catalogue imprint; with 2d ending 1983 volumes may overlap within 1983. Volumes numbered consecutively in the series (at least to v.210 in the catalogue note). Current status not shown by any fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| third series start | 1983 | <https://sclqld.org.au/catalogue/records/64122> | Publication details: Ontario : Candada Law Book Ltd., 1983- |
| second series ends 1983 | 1971-1983 | <https://www.courthouselibrary.ca/catalogue-item?cid=8412> | Publication Date: 1983 Publication Date Range: 1971-1983. |
| third series continues the second series | CCC (2d) -> CCC (3d) | <https://sclqld.org.au/catalogue/records/64122> | Continues: Canadian criminal cases (second series) : a series of reports of important decisions in criminal and quasi-criminal cases. |
| numbering | volume 1983- | <https://www.courthouselibrary.ca/catalogue-item?cid=26018> | Paper parts issued weekly. Vols. 1-34 (1 v.); v. 35-75 (1 v.); v. 76-100 (1 v.); v. 101-127 (1 v.); v. 128-157 (1 v.); v. 158-188 (1 v.); v. 189-210 (1 v.). |

Not established: still publishing not proved.

### WWR (wwr)

Edition WWR covers the first series (1911-1950) and the 1971- series; WWR (NS) is 1951-1970. No page fetched states when the 1971- series ended; UNB figure is a holdings list. In the 1971- series volumes restart each year.

| Fact | Value | Source | Quote |
|---|---|---|---|
| series boundaries | first series 1911-1950; New series 1951-1970; current series from 1971 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | First series ceased with issue for Dec. 30, 1950; New series began in 1951, ceased with issue for Dec. 26, 1970; current series began in 1971. |
| start | 1911 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Publisher: Carswell Publication Date: 1911 Subject(s): Law reports, digests, etc. - Canada, Western. Notes: Cited: W.W.R. |
| vol. 1 | 1911 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Kamloops 1911/1912 v. 1 |
| vol. 10 | 1916 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Kamloops 1916 v. 10 |
| numbering | volume 1911-1916 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Vols. for 1911-Sept. 1916 called v. 1-10 |
| numbering | year_and_volume 1917-1950 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Numbering begins each year with v. 1, 1917-1950; 1971- |
| numbering | year_and_volume 1971- | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Numbering begins each year with v. 1, 1917-1950; 1971- |

Not established: end of WWR 1971- series 2003 (library holdings range only).

### DLR (3d) (dlr)

Sources agree on 1969-1984, 150 volumes, continuous volume numbers.

| Fact | Value | Source | Quote |
|---|---|---|---|
| vol. 1 | 1969 | <https://www.courthouselibrary.ca/catalogue-item?cid=2556> | vol. 1 (1969)-v. 150 (1984) |
| vol. 150 | 1984 | <https://search.library.wisc.edu/catalog/999618282902121> | 3rd ser., v. 1 (1969)-v. 150 (1984) |
| numbering | volume 1969-1984 | <https://www.courthouselibrary.ca/catalogue-item?cid=2556> | A subject index to D.L.R. (3d), vol. 1 (1969)-v. 150 (1984). |

Not established: range of 3rd series 1969-1984; registry example (1968), 1 D.L.R. (3d) 354 year precedes verified 1969 start.

### All ER (aller)

Start 1936 stated by Wisconsin (Butterworth 1936-) and Lexis+ online coverage; sclqld imprint 1948 is the Law Journal merger. Registry example [1935] All E.R 1 falls in the All ER Reprint (vols 1835-1935), not the 1936- series. Whether citations are year plus volume-in-year is not stated by a fetched page (only 'Published in 2 vols. per year'), so numbering is left empty. No page shows a recent volume.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start (Butterworth, as All England law reports annotated 1936-46) | 1936 | <https://search.library.wisc.edu/catalog/999618793502121> | Other Titles All England law reports annotated 1936-46 All England law reports 1947 All England law reports, incorporating the Law times reports 1948-49 Format Journals, Magazines, Newspapers Language English |
| start per same record | 1936 | <https://search.library.wisc.edu/catalog/999618793502121> | Publication London : Butterworth Publication Date 1936- Physical Details volumes ; 24 cm |
| online coverage start | 1936 | <https://www.sclqld.org.au/catalogue/records/19719> | Available online Online via Lexis+ (1936-) |
| All ER Reprint (All ER Rep) covers 1558-1935, published 1957-1971 | 1558-1935 | <https://www.sclqld.org.au/catalogue/records/19720> | Publication details: London : Butterworth, 1957-1971. Edition: Record id: 19720 Added title: Law times reports. Author: Format: Series: Subject: Law reports, digests, etc. -- Great Britain. Contents: Summary: Vols. for 1558-1834 selected fr |

Not established: imprint date of the library's main record 1948; volumes per year 2 per year.

### OR (3d) (or)

Third series open-ended in the Wisconsin catalogue; no page fetched proves a recent volume or an end. The 1988 example is probably a miscopy (not inferred; flagged).

| Fact | Value | Source | Quote |
|---|---|---|---|
| third series start | 1991 | <https://search.library.wisc.edu/catalog/999620353502121> | Jan. 11, 1963-Dec. 21, 1973; 2d ser., v. 1-v. 75, pt. 10 ; Jan. 4, 1974-Feb. 8, 1991; 3rd ser., v. 1, pt. 1- ; Feb. 22, 1991- |
| third series publisher record | 1991 | <https://www.courthouselibrary.ca/catalogue-item?cid=7053> | Publisher: Butterworths Publication Date: 1991 |
| numbering | volume 1991- | <https://search.library.wisc.edu/catalog/999620353502121> | 3rd ser., v. 1, pt. 1- ; Feb. 22, 1991- |

Not established: holdings-only range 1882-2015; registry example (1988), 39 O.R. (3d) 140 1988 precedes verified 1991 start; second series range 1974-1991.

### NSR (2d) (nsr)

Only one catalogue states 1970 as a bare publication date; left unverified. Volumes appear numbered consecutively (holdings list v. 1, v. 2, ...). No end shown.

| Fact | Value | Source | Quote |
|---|---|---|---|
| predecessor Maritime Provinces Reports range | 1931-1968 | <https://www.courthouselibrary.ca/catalogue-item?cid=1252> | Publication Date: 1968 Publication Date Range: 1931-1968. |
| first series NSR 1-60 range | 1855-1929 | <https://www.courthouselibrary.ca/catalogue-item?cid=11315> | Publication Date: 1929 Publication Date Range: 1855-1929. |

Not established: 2d series catalogue imprint date (single source, no range) 1970; holdings-only range of Nova Scotia Reports 1834-2016.

### DLR (dlr)

Edition 'DLR' = first/old and new series before (2d). Wisconsin shows 1923 onward numbered 'v.1' within each year to v.5 in 1955, i.e. year plus volume; the string '1923, v. 1-1955, v. 5' is the library's compact form and the per-year restart is read from it, not stated in prose.

| Fact | Value | Source | Quote |
|---|---|---|---|
| first series range (Old plus New series) | 1912-1955 | <https://www.courthouselibrary.ca/catalogue-item?cid=1129> | Publication Date: 1955 Publication Date Range: 1912-1955. Additional Corporate Author: |
| old series/new series split | old series vols 1-70 (1912-1922); new series 1923-1955 vols 1-5 per year-set ending 1955 | <https://sclqld.org.au/catalogue/records/19861> | SCLQ print holdings: vol. 1 (1912) - vol. 70 (1922) ; new series, vol. 1 (1923) - vol. 5 (1955) ; 2nd series, vol. 1 (1956) - volume 70 (1968) |
| citation of first volume is dated 1912 | 1912 | <https://www.courthouselibrary.ca/catalogue-item?cid=1129> | together with Canadian cases appealed to the Privy Council'-t.p., (1912) v. 1. |
| vol. 1 | 1912 | <https://search.library.wisc.edu/catalog/999618282902121> | Vol. 1 (1912)-v. 70 (1922) |
| vol. 70 | 1922 | <https://search.library.wisc.edu/catalog/999618282902121> | Vol. 1 (1912)-v. 70 (1922) |
| numbering | volume 1912-1922 | <https://search.library.wisc.edu/catalog/999618282902121> | Vol. 1 (1912)-v. 70 (1922) |
| numbering | year_and_volume 1923-1955 | <https://search.library.wisc.edu/catalog/999618282902121> | 1923, v. 1-1955, v. 5 |

### ACWS (3d) (acws)

Case-summary service, not law reports proper. Start 1987 from library range; no end or recent volume proved. Registry example (1991) 27 ACWS (3d) is consistent.

| Fact | Value | Source | Quote |
|---|---|---|---|
| third series range | 1987- | <https://www.courthouselibrary.ca/catalogue-item?cid=5648> | Publication Date: 1987 Publication Date Range: 1987- Subject(s): Law reports, digests, etc. - Canada Civil law - Canada - Digests. Notes: Cited: A.C.W.S. (3d). |
| second series range | 1980-1986 | <https://www.courthouselibrary.ca/catalogue-item?cid=11457> | Publication Date: 1986 Publication Date Range: 1980-1986. |

Not established: still publishing not proved.

### FTR (ftr)

Two library pages give different starts (1986 vs 1980) and the UNB range is a holdings list. Authoritative would be Maritime Law Book (publisher, closed 2016); not reached. The registry examples start (1987) 16 FTR.

Not established: UNB holdings listing 1980-2002; library catalogue imprint 1986.

### BCLR (2d) (bclr)

Volume numbers appear consecutive within the series in holdings (v. 1, v. 2 ...). Ranges are catalogue imprint years; a volume may fall in the adjacent year.

| Fact | Value | Source | Quote |
|---|---|---|---|
| second series range | 1986-1995 | <https://www.courthouselibrary.ca/catalogue-item?cid=5644> | Publication Date: 1995 Publication Date Range: 1986-1995. Subject(s): Law reports, digests, etc. - British Columbia. Notes: Cited: B.C.L.R. (2d). |
| first series range (ends where 2d begins) | 1977-1986 | <https://www.courthouselibrary.ca/catalogue-item?cid=5660> | Publication Date: 1986 Publication Date Range: 1977-1986. |

Not established: registry example (1976), 10 BCLR (2d) 172 1976 precedes verified 1986 start.

### NR (nr)

Maritime Law Book's National Reporter; end not stated by any publisher or catalogue page. UNB 2003 is holdings.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1974 | <https://www.courthouselibrary.ca/catalogue-item?cid=6999> | Publisher: Maritime Law Book Co. Publication Date: 1974 |
| start (second page) | 1974 | <https://libguides.bodleian.ox.ac.uk/law-can/cases> | National Reporter (1974 - ) Cw Can 100 N20 |

Not established: holdings range 1974-2003.

### OR (2d) (or)

Wisconsin shows the first series ending Dec. 21, 1973 and the 2d series beginning Jan. 4, 1974; that string is read as consecutive ranges. Courthouse Libraries separately dates the earlier Ontario Reports 1932-1973 (cid 1269).

| Fact | Value | Source | Quote |
|---|---|---|---|
| second series dates (issue level) | Jan. 4, 1974 - Feb. 8, 1991 | <https://search.library.wisc.edu/catalog/999620353502121> | Jan. 11, 1963-Dec. 21, 1973; 2d ser., v. 1-v. 75, pt. 10 ; Jan. 4, 1974-Feb. 8, 1991; 3rd ser., v. 1, pt. 1- ; Feb. 22, 1991- |
| numbering | volume 1974-1991 | <https://search.library.wisc.edu/catalog/999620353502121> | 2d ser., v. 1-v. 75, pt. 10 ; Jan. 4, 1974-Feb. 8, 1991 |

Not established: registry example (1973), 1 O.R. (2d) 113 1973 precedes verified Jan 1974 start; second series range 1974-1991.

### ER (er)

start/end here are the reprint's publication years (1900-1932), NOT the years of cited cases. For a citation-year check on '(year) N ER page' the case year may lie anywhere in 1220-1867 per the volume contents. Volumes are continuous (1-178, 176 online).

| Fact | Value | Source | Quote |
|---|---|---|---|
| reprint publication years | 1900-1932 | <https://www.sclqld.org.au/catalogue/records/90710> | The English Reports reprint, 1220-1865, was issued in 178 volumes, 1900-32 and is in general a straight reprint in which the original footnotes are given. |
| years of the decisions reprinted (use these for citation-year checks, not 1900-1932) | 1220-1865 | <https://www.sclqld.org.au/catalogue/records/90710> | Contains decisions of all English courts from 1220 to the commencement of the official Law reports in 1866. |
| volume groups by court and years | vols 1-11 HL 1694-1865 ... vols 170-176 Nisi Prius 1688-1867 | <https://www.sclqld.org.au/catalogue/records/90710> | 1-11. House of Lords (1694 to 1865) 12-20. Privy Council (including Indian Appeals) (1809 to 1865) 21-47. Chancery (including Collateral Reports) (1557 to 1865) |
| numbering | volume 1900-1932 | <https://www.sclqld.org.au/catalogue/records/19714> | Phys. description: 178 volumes ; 26 cm |

### QB (qb)

Several distinct series use QB: LR QB (1865-75), QBD (1875-90), QB (1891-1900 and 1952/3-2022/3). Registry example (1852) 18 Q.B. 93 predates all of these and is not this series (likely Adolphus & Ellis new series QB, not proved by a fetched page). The 1865 start is the ICLR online first-volume year; the imprint is 1866.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Law Reports Court of Queen's Bench (LR QB) | 1865/66-1875, 10 volumes | <https://www.sclqld.org.au/catalogue/records/20121> | Online via ICLR online vol1 (1865) - vol10(1875) Holdings Clear filters Loading holdings... Record details Publication details: London : Printed for the Council of Law Reporting by W. Clowes and Sons, 1866-1875. |
| Queen's Bench Division (QBD) 1875-1890 | vols 1-25 | <https://www.sclqld.org.au/catalogue/records/20126> | The law reports : Queen's Bench Division, 1875-1890 / edited by James Redfoord Bulwer. Copy direct link Share Available online Online via ICLR online vol1 (1875) - vol25(1890) |
| QB 1891-1900 | 1891-1900 | <https://www.sclqld.org.au/catalogue/records/24159> | The law reports : Queen's Bench Division, 1891-1900. |
| KB (King's Bench) in place of QB | 1901-1952 | <https://www.sclqld.org.au/catalogue/records/87786> | The law reports : King's Bench Division, 1901-1952. |
| QB resumes 1952/53; ends under that name 2022/23 | 1953-2023 | <https://www.sclqld.org.au/catalogue/records/20131> | Continues: Law reports. King's Bench Division [1901-1952] Continued by: Law reports. King's Bench Division [2023]- |
| 1952 to 1955 volume title wobble | 1952 v2-1955 v1 Queen's Bench | <https://www.courthouselibrary.ca/catalogue-item?cid=10487> | 1952, volume 2-1955, volume 1, Law reports of the Incorporated Council of Law Reporting. Queen's Bench Division; 1955, volume 2 - , Law reports. Queen's Bench Division |
| numbering | year_and_volume 1891- | <https://www.iclr.co.uk/history/> | In 1891 the Council introduced the simplified arrangement of dated annual volumes in four series which continues to this day |

Not established: dating ambiguity 2022 vs 2023 authors 1953-2022; publication 1953-2023.

### CCC (2d) (ccc)

Boundaries are catalogue imprint ranges (2d: 1971-1983). Registry example (1971) 1 CCC (2d) 463 is consistent. Numbering not stated by a fetched page for this series beyond consecutive-volume holdings; left empty.

| Fact | Value | Source | Quote |
|---|---|---|---|
| second series range | 1971-1983 | <https://www.courthouselibrary.ca/catalogue-item?cid=8412> | Publisher: Canada Law Book Publication Date: 1983 Publication Date Range: 1971-1983. Subject(s): Criminal law - Canada - Cases. Law reports, digests, etc. - Canada Notes: Cited: CCC (2d). |
| New Series (CCC) before it | 1963-1970 | <https://sclqld.org.au/catalogue/records/64121> | Publication details: Toronto : Law Book Co., 1963-1970. |
| first series Canadian Criminal Cases annotated starts 1898 | 1898 | <https://www.canadiana.ca/view/oocihm.8_02258> | Canadian criminal cases annotated : Vol. I (1898) 1898 Canadian criminal cases annotated : Vol. II (1899) 1899 Canadian criminal cases annotated : Vol. III (1900) 1900 |
| second series follows new series | CCC (new series) -> CCC (2d) | <https://sclqld.org.au/catalogue/records/64121> | Continued by: Canadian criminal cases (second series) : a series of reports of important decisions in criminal and quasi-criminal cases. |
| third series begins | 1983 | <https://sclqld.org.au/catalogue/records/64122> | Publication details: Ontario : Candada Law Book Ltd., 1983- |

### WLR (wlr)

ICLR present-tense page supports current. Years in which year+volume citation applied are not stated for the earliest years; numbering start left null. Pre-1953 W.L.R. cites in the registry belong to other series (Western Law Reporter 1905-1916 is a holdings range).

| Fact | Value | Source | Quote |
|---|---|---|---|
| start | 1953 | <https://www.iclr.co.uk/products/law-report-series/the-weekly-law-reports/> | Published since 1953, the Weekly Law Reports (WLR) provide the best way to keep up to date with all cases of general interest heard in the superior and appellate courts. |
| succeeded Weekly Notes in 1953 | 1953 | <https://www.iclr.co.uk/history/> | It was superseded in 1953 by the Weekly Law Reports (WLR) |
| catalogue | 1953- | <https://www.sclqld.org.au/catalogue/records/19968> | Publication details: London : Incorporated Council of Law Reporting, 1953- Edition: |
| volume 4 cited by year, volume, abbreviation and case number | current | <https://www.iclr.co.uk/products/law-report-series/the-weekly-law-reports/> | Case reports appearing in volume 4 are cited by year, volume, series abbreviation and case number |
| registry examples (1911) 18 W.L.R., (1930) 3 W.L.R. | not the Weekly Law Reports (Western Law Reporter existed) | <https://unb.ca/fredericton/law/library/about/law-reports-locator.html> | Western Law Reporter (1905–1916): 162 |
| numbering | year_and_volume None- | <https://www.iclr.co.uk/products/law-report-series/the-weekly-law-reports/> | Case reports appearing in volumes 1 to 3 are cited by year, volume, series abbreviation and page number |

### AR (ar)

1976 vs 1977; publisher not reached. Registry example (1976) 2 A.R. 453 would fit 1976.

| Fact | Value | Source | Quote |
|---|---|---|---|
| companion index imprint | [1979]- | <https://www.courthouselibrary.ca/catalogue-item?cid=2290> | Publication Date: 1979 Publication Date Range: [1979]- |

Not established: Bodleian print holdings start 1977; library catalogue imprint 1976.

### DLR (2d) (dlr)

Three catalogues agree: 70 volumes, 1956-1968.

| Fact | Value | Source | Quote |
|---|---|---|---|
| range | 1956-1968 | <https://www.courthouselibrary.ca/catalogue-item?cid=1270> | Publication Date: 1968 Publication Date Range: 1956-1968. |
| vol. 1 | 1956 | <https://search.library.wisc.edu/catalog/999618282902121> | 2nd ser., v. 1 (1956)-v. 70 (1968) |
| vol. 70 | 1968 | <https://sclqld.org.au/catalogue/records/19861> | 2nd series, vol. 1 (1956) - volume 70 (1968) |
| numbering | volume 1956-1968 | <https://search.library.wisc.edu/catalog/999618282902121> | 2nd ser., v. 1 (1956)-v. 70 (1968) |

Not established: registry example (1955), 1 D.L.R. (2d) 179 1955 precedes 1956 start.

### DTC (dtc)

Registry examples cite as '(1974) 74 DTC 6315' i.e. two-digit year as volume; no fetched page states that numbering scheme. Left unverified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| coverage note | Tax Court of Canada decisions included from 1983 | <https://www.courthouselibrary.ca/catalogue-item?cid=11394> | Includes decisions from various courts in Canada, including the Supreme Court, and from 1983- the Tax Court of Canada. |

Not established: CCH Dominion Tax Cases catalogue date (bare date, single source) 1940.

### CLR (clr)

Could not settle. Construction Law Reports (Carswell, 1983-) appeared only in a search snippet from a blocked site; no raw page fetched. Likely a misattribution: the examples' volume/year pairs (51 CLR in 1934) are Australian Commonwealth Law Reports. Not proved.

### BCLR (bclr)

Holdings list 70 volumes (v. 1-70) for 1977-1986. Registry examples (1978) 7 BCLR consistent. UNB/Bodleian ranges starting 1867/1930 describe older BC Reports, not the 1977 BCLR.

| Fact | Value | Source | Quote |
|---|---|---|---|
| first series range | 1977-1986 | <https://www.courthouselibrary.ca/catalogue-item?cid=5660> | Publisher: Carswell Publication Date: 1986 Publication Date Range: 1977-1986. Subject(s): Law reports, digests, etc. - British Columbia. Notes: Cited: B.C.L.R. |
| 2d series 1986-1995; 3d 1995-2002; 4th 2002-2010; 5th began 2010 | see ranges | <https://www.courthouselibrary.ca/catalogue-item?cid=10898> | Publication Date Range: 2002-2010. Subject(s): Law reports, digests, etc. - British Columbia. Notes: Cited: B.C.L.R. (4th). |
| fifth series start | April 2010 | <https://www.courthouselibrary.ca/catalogue-item?cid=16015> | Description based on volume 1, part 1 (Apr. 2010) |
| predecessor BC Reports/case reports 1948-1976 | 1948-1976 | <https://courthouselibrary.ca/node/1637> | BC case reports (1948-1976) \| Courthouse Libraries BC |

Not established: UNB holdings entry for BCLR (not the 1977 series run) 1930-2014.

### BCAC (bcac)

Start year not stated as a series start; left unverified. Holdings are volumes 1-100 (index v. 1-100) with no years.

Not established: catalogue imprint date (bare date, single source) 1991; companion digest begins v. 1/10 (1991/1992) 1991/1992.

### WWR (NS) (wwr)

'Called also' is ambiguous: the volumes may be cited by year-set plus number; the page gives only the 1-75 numbering. Treat numbering as continuous 1-75.

| Fact | Value | Source | Quote |
|---|---|---|---|
| new series boundaries | 1951 to Dec. 26, 1970 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | First series ceased with issue for Dec. 30, 1950; New series began in 1951, ceased with issue for Dec. 26, 1970; current series began in 1971. |
| numbering of the new series | volumes numbered 1-75 over 1951-1970 (also the library says 'called also') | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | Vols. for 1911-Sept. 1916 called v. 1-10; v. for 1951-1970 called also v. 1-75. |
| numbering | volume 1951-1970 | <https://www.courthouselibrary.ca/catalogue-item?cid=26021> | v. for 1951-1970 called also v. 1-75. |

### BCLR (3d) (bclr)

Catalogue imprint ranges.

| Fact | Value | Source | Quote |
|---|---|---|---|
| third series range | 1995-2002 | <https://www.courthouselibrary.ca/catalogue-item?cid=7690> | Publication Date: 2002 Publication Date Range: 1995-2002. Subject(s): Law reports, digests, etc. - British Columbia. Notes: Cited: B.C.L.R. (3d). |
| second series ends 1995 | 1986-1995 | <https://www.courthouselibrary.ca/catalogue-item?cid=5644> | Publication Date Range: 1986-1995. |
| 4th series follows | 2002-2010 | <https://www.courthouselibrary.ca/catalogue-item?cid=10898> | Publication Date Range: 2002-2010. |

Not established: registry example (1985), 26 B.C.L.R. (3d) 353 1985 precedes verified 1995 start.

### CPR (3d) (cpr)

Imprint ranges; boundary years may overlap by a volume. Volumes numbered consecutively (not stated explicitly).

| Fact | Value | Source | Quote |
|---|---|---|---|
| third series range | 1985-1999 | <https://www.courthouselibrary.ca/catalogue-item?cid=8714> | Publication Date Range: 1985-1999. Subject(s): Patent laws and legislation - Canada - Cases. Copyright - Canada - Cases. Trademarks - Canada - Cases. Law reports, digests, etc. - Canada Notes: Cited: C.P.R. (3d). |
| second series | 1971-1984 | <https://www.courthouselibrary.ca/catalogue-item?cid=931> | Publication Date Range: 1971-1984. Subject(s): Law reports, digests, etc. - Canada Patent laws and legislation - Canada - Cases. Copyright - Canada - Cases. Trademarks - Canada - Cases. Notes: Supersedes Canadian patent reporter. Cited: CPR |
| first series | 1941-1971 | <https://www.courthouselibrary.ca/catalogue-item?cid=3056> | Publication Date Range: 1941-1971 |
| fourth series imprint | 1999 | <https://www.courthouselibrary.ca/catalogue-item?cid=8715> | Publisher: Canada Law Book Publication Date: 1999 Subject(s): Patent laws and legislation - Canada - Cases. |

Not established: registry example (1984), 3 C.P.R. (3e) 81 1984 precedes 1985 start (series ranges are imprint years; 2d ends 1984).

### RJQ (rjq)

1986 matches the registry. Ten parts per year (cid 9127 note), cited by year only. Earlier Court of Appeal reports (1921-75, 1970-85) also abbreviate RJQ/R.J.Q.; a registry using 1986 would reject those pre-1986 cites. Current status not proved.

| Fact | Value | Source | Quote |
|---|---|---|---|
| start (merged series) | 1986 | <https://www.courthouselibrary.ca/catalogue-item?cid=9127> | Publication Date: 1988 Publication Date Range: 1986-1988. Additional Corporate Author: |
| abbreviation R.J.Q. used for Court of Appeal reports before 1986 | 1921-75 | <https://sclqld.org.au/catalogue/records/19894> | Publication details: Montreal: Editeur officiel du Quebec, 1921-75. |
| numbering | year 1986- | <https://www.courthouselibrary.ca/catalogue-item?cid=9127> | Cited as [1986] R.J.Q., etc |

Not established: merger of earlier courts' recueils Cour d'appel 1970-1985; Cour supérieure 1967-1985; Cour provinciale 1976-1985; separate 1989- record 1989-.

### Sask R (saskr)

1979 vs 1980; holdings lists are not series runs. Registry example (1977), 6 Sask. R. 74 is earlier than all of these (probably a miscopy; flagged, not inferred).

Not established: Bodleian holdings range 1980-2004; UNB holdings range 1980-2016; library catalogue imprint 1979.

### CPC (6th) (cpc)

No fetched page states the run of any Carswell Practice Cases series. Nothing verified.

### OAC (oac)

Only a library holdings range was found; the publisher/national-library run was not established.

Not established: UNB Law Library holdings range for Ontario Appeal Cases (holdings, not the series run) 1984-1999.

### CCC (NS) (ccc)

Supreme Court Library Queensland catalogue record 64121 (library catalogue). Numbering style not stated.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Canadian criminal cases (new series) publication dates | 1963-1970 | <https://sclqld.org.au/catalogue/records/64121> | Toronto : Law Book Co., 1963-1970. |
| Extent | 30 volumes | <https://sclqld.org.au/catalogue/records/64121> | Phys. description: 30 volumes |
| Continues first series, continued by second series | CCC annotated -> new series -> second series | <https://sclqld.org.au/catalogue/records/64121> | Continues: Canadian criminal cases annotated. Continued by: Canadian criminal cases (second series) |

Not established: Second series (CCC (2d)) dates not stated on any fetched page.

### CPR (4th) (cpr)

WARNING: the CiNii catalogue dates volume 1 of the 4th series 2000, but the registry has a citation example from 1999 for a 4th-series volume 2 (not used as evidence). A catalogue year can be the cataloguer's first-issue date; start left null (not asserted) until a publisher page confirms. Third series ended with v. 87 ([1999]).

| Fact | Value | Source | Quote |
|---|---|---|---|
| Fourth series first volume per CiNii record | Vol. 1 (2000) | <https://ci.nii.ac.jp/ncid/AA12174343?l=en> | Canadian patent reporter. Fourth series Canada Law Book, 2000- Vol. 1 (2000)- |
| Third series last volume | v. 87 ([1999]) | <https://ci.nii.ac.jp/ncid/AA1121182X?l=en> | Canadian patent reporter. Third series Canada Law Book, -c1999 -v. 87 ([1999]) |
| Third series first volume (LC MARC) | Vol. 1 (Jan. 1985) | <https://ci.nii.ac.jp/ncid/AA1121182X?l=en> | VLYR of LC MARC: Vol. 1 (Jan. 1985)- |

### RFL (rfl)

First series RFL = 1971-1978, vols 1-30, per CiNii bibliographic record (NII, Japan). Boundaries of 2d (1978-1986) and 3d (1986-) shown in facts. RFL (4th) start not established.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Second series volumes | Vol. 1 (1978)-50 (1986) | <https://ci.nii.ac.jp/ncid/AA10807835?l=en> | Reports of family law. 2d ser Carswell, 1978-1986 Vol. 1 (1978)-50 (1986) |
| Third series starts | Vol. 1 (1986) | <https://ci.nii.ac.jp/ncid/AA10850040?l=en> | Reports of family law. 3d ser Carswell Vol. 1 (1986)- |
| vol. 1 | 1971 | <https://ci.nii.ac.jp/ncid/AA00375756?l=en> | Vol. 1 (1971) |
| vol. 30 | 1978 | <https://ci.nii.ac.jp/ncid/AA00375756?l=en> | v. 30 (1978) |
| numbering | volume 1971-1978 | <https://ci.nii.ac.jp/ncid/AA00375756?l=en> | Reports of family law Carswell Vol. 1 (1971)-v. 30 (1978) |

### Nfld & PEIR (nfldpeir)

Style guide period 1971- is open-ended and does not prove the series is still current or when it ended; UNB range is a holdings range.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Series start (NZ Law Style Guide period column) | 1971 | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_3.html> | Nfld & PEIR Newfoundland & Prince Edward Island Reports 1971– |

Not established: UNB Law Library holdings range (holdings, not the series run) 1971-2016.

### CNLR (cnlr)

Predecessor: Canadian native law bulletin (page: "Continues: Canadian native law bulletin."). No page fetched states an end year or that CNLR is current.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Series began | 1979 | <https://ci.nii.ac.jp/ncid/AA10875008?l=en> | LC MARC: Began publication in 1979 |

### CR (3d) (cr)

CiNii record (NII, Japan). Other sources (Cardiff index, blocked) reportedly say 1978-1990; not verified.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Cumulative index | v. 51-80 issued 1991 | <https://ci.nii.ac.jp/ncid/AA10840364?l=en> | Consolidated index covering v. 51-80 issued in 1991 |
| vol. 1 | 1978 | <https://ci.nii.ac.jp/ncid/AA10840364?l=en> | Vol. 1 (1978) |
| vol. 80 | 1991 | <https://ci.nii.ac.jp/ncid/AA10840364?l=en> | v. 80 (1991) |
| numbering | volume 1978-1991 | <https://ci.nii.ac.jp/ncid/AA10840364?l=en> | Criminal reports. Third series Carswell, 1978-1991 Vol. 1 (1978)-v. 80 (1991) |

### OR (or)

Plain "OR" covers the first Ontario Reports 1882-1900 (vols 1-32) per the catalogue contents note; the title was reused later (name resumed after Ontario Law Reports/Ontario Weekly Notes), so end left null. On the 1900 vs 1901 start of Ontario Law Reports the catalogue (Supreme Court Library Queensland) is the more authoritative than a style guide.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Ontario Reports (first series) volumes and successor title | 1882-1900 Ontario Reports; 1901-1931 Ontario Law Reports | <https://sclqld.org.au/catalogue/records/19886> | Contents: 1882-1900, Ontario Reports --1901-1931, Ontario Law Reports. |
| Catalogue publication date | 1882- | <https://sclqld.org.au/catalogue/records/19886> | Agincourt, Ont. : Canada Law Book, 1882-. |
| Ontario Weekly Notes (1921-1962) continued by Ontario reports | OWN 1921-1962 -> Ontario reports | <https://sclqld.org.au/catalogue/records/19888> | Toronto : Canada Law Book, 1921-1962. |
| NZ Law Style Guide periods for OR (2d) and OR (3d) | OR (2d) 1973-1990; OR (3d) 1991- | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_3.html> | OR (2d) Ontario Reports (2d) 1973–1990 OR (3d) Ontario Reports (3d) 1991– |

Not established: NZ Law Style Guide dates Ontario Law Reports 1900-1931 (disagrees with catalogue 1901) 1900-1931; Online coverage of Ontario Reports (coverage, not run) 1931-present.

### Man R (2d) (manr)

First series ended 1961 per NZ Law Style Guide; second series start not stated on any fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Manitoba Reports (first series) period | 1883-1961 | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_3.html> | Man R Manitoba Reports 1883–1961 |

Not established: UNB holdings range, apparently second series (holdings, not run) 1975-2015.

### CCC (ccc)

End 1962 is bracketed in the catalogue (cataloguer-supplied), and agrees with the new series record starting 1963. Numbering (continuous volumes) not stated on a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| First series (Canadian criminal cases annotated) publication dates; end year is cataloguer-bracketed | 1898-[1962] | <https://sclqld.org.au/catalogue/records/63286> | Toronto : Canada Law Journal Company, 1898-[1962] |
| Variant title shows C.C.C. is the first series abbreviation | Canadian criminal cases C.C.C. | <https://sclqld.org.au/catalogue/records/63286> | Variant title: Canadian criminal cases C.C.C. |
| Collection of all series from 1898 (Westlaw CriminalSource blog) | all series, 1898 to the present | <https://yorku.ca/osgoode/libblog/?p=1209> | from 1898 to the present |
| Third series (CCC (3d)) begins | 1983 | <https://sclqld.org.au/catalogue/records/64122> | Ontario : Candada Law Book Ltd., 1983- |

Not established: McMaster coverage (online coverage, not run) 1898-present.

### Ch (ch)

current=true rests on the publisher's present-tense statement, not on a dated recent volume. Numbering (by year) not stated on a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Law Reports Chancery Division period | 1891- | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_4.html> | Ch Chancery Division 1891– |
| Predecessor Ch D | 1875-1890 | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_4.html> | Ch D Chancery Division 1875–1890 |
| ICLR (publisher) states Ch is one of its official Law Reports series (present tense, page footer 2026) | current | <https://www.iclr.co.uk/about/> | Where a judgment is reported in the official Law Reports (AC, QB, Ch, Fam) published by the Incorporated Council of Law Reporting for England and Wales, that report must be cited. |
| ICLR page footer date | 2026 | <https://www.iclr.co.uk/about/> | © 2026 The Incorporated Council of Law Reporting for England & Wales |

### CR (4th) (cr)

End year of 4th series not stated; the 5th series begins 1997.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Fifth series starts (so 4th ended by then); last 4th-series volume/year not stated | Vol. 1 (1997) | <https://ci.nii.ac.jp/ncid/AA11176547?l=en> | Cinquième série Carswell, 1997- Vol. 1 (1997)- |
| vol. 1 | 1991 | <https://ci.nii.ac.jp/ncid/AA10840386?l=en> | Quatrième série Carswell, 1991- Vol. 1 (1991)- |
| numbering | volume 1991- | <https://ci.nii.ac.jp/ncid/AA10840386?l=en> | Criminal reports. Fourth series = Recueil de jurisprudence en droit criminel. Quatrième série Carswell, 1991- Vol. 1 (1991)- |

### LAC (4th) (lac)

Series boundaries (4th series start) not established by a fetched page.

Not established: McMaster online coverage for L.A.C. (coverage, not series run) 1948-present.

### CR (NS) (cr)

Not established. No fetched page gives dates for Criminal Reports, New Series (Cardiff Index lists 1967-1978 but is blocked to raw fetch; not recorded). The third series begins 1978 (see CR (3d)).

### CTC (ctc)

Nothing verified for Canada Tax Cases.

### CPC (4th) (cpc)

No fetched page states the run of any Carswell Practice Cases series. Nothing verified.

### Alta LR (3d) (altalr)

Third series dates not established.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Alberta Law Reports first series period | 1908-1932 | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_3.html> | Alta LR Alberta Law Reports 1908–1932 |

Not established: UNB holdings range (probably 2d series; holdings only) 1977-1990.

### CBR (3d) (cbr)

3d series start 1991 from CiNii. CBR (NS) ran at least 1960-1990 per index notes (index date ranges, not stated series boundaries). End of 3d series not established.

| Fact | Value | Source | Quote |
|---|---|---|---|
| New series volume/year index ranges | vols 1-36 = 1960-1981; vols 61-76 = 1986-1990 | <https://ci.nii.ac.jp/ncid/AA11012696?l=en> | New ser., vol. 1-36, 1960-1981. |
| New series later index range | vols 61-76 = 1986-1990 | <https://ci.nii.ac.jp/ncid/AA11012696?l=en> | New ser., vol. 61-76, 1986-1990 |
| vol. 1 | 1991 | <https://ci.nii.ac.jp/ncid/AA10955311?l=en> | Vol. 1 (1991) |
| numbering | volume 1991- | <https://ci.nii.ac.jp/ncid/AA10955311?l=en> | Canadian bankruptcy reports. Third series Carswell [etc.], c1991- Vol. 1 (1991)- |

### RFL (4th) (rfl)

Nothing verified for the fourth series; only the third series start (1986) is on a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Third series start (4th series presumably after it; 4th start not stated on any fetched page) | Vol. 1 (1986) | <https://ci.nii.ac.jp/ncid/AA10850040?l=en> | Reports of family law. 3d ser Carswell Vol. 1 (1986)- |

### CPC (7th) (cpc)

No fetched page states the run of any Carswell Practice Cases series. Nothing verified.

### CR (5th) (cr)

End year of the 5th series and whether current: not established.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Continues fourth series | yes | <https://ci.nii.ac.jp/ncid/AA11176547?l=en> | Continues: Criminal reports. Fourth series |
| vol. 1 | 1997 | <https://ci.nii.ac.jp/ncid/AA11176547?l=en> | Cinquième série Carswell, 1997- Vol. 1 (1997)- |
| numbering | volume 1997- | <https://ci.nii.ac.jp/ncid/AA11176547?l=en> | Cinquième série Carswell, 1997- Vol. 1 (1997)- |

### NZLR (nzlr)

Contradiction: Melbourne libguide says v.1 1881; two catalogues (SCLQ, IALS) say 1883. Catalogues treated as authoritative. current not proven by a dated recent volume.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Publication dates per library catalogue (authoritative) | 1883- | <https://sclqld.org.au/catalogue/records/19687> | Wellington : New Zealand Council of Law Reporting, 1883-. |
| IALS records NZ reports from 1883 | 1883 onwards, active as at 2013 | <https://resources.ials.sas.ac.uk/node/683533> | 1 title containing court reports for New Zealand from 1883 onwards. |
| IALS accrual status | Active as at 2013 | <https://resources.ials.sas.ac.uk/node/683533> | Accrual status: Active as at 2013 |

Not established: University of Melbourne guide gives 1881 for volume 1 1881.

### NBR (2d) (nbr)

NZ Law Style Guide period column, open ended: end/current not established.

| Fact | Value | Source | Quote |
|---|---|---|---|
| New Brunswick Reports (2d) period | 1969- | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_3.html> | NBR (2d) New Brunswick Reports (2d) 1969– |
| First series period | 1825-1929 | <https://lawfoundation.org.nz/style-guide/nzlsg_appendix_3.html> | NBR New Brunswick Reports 1825–1929 |

### FCR (fcr)

Volumes are numbered 1-4 within each year (4 volumes a year), cited by year. current left null: newest volume visible on the record is 2023. Predecessor (Federal Court Reports) is cited as FC; FCR began with 2004 vol. 1 per this catalogue record.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Publisher and start | Ottawa : Office of the Commissioner for Federal Judicial Affairs, [2004]- | <https://publications.gc.ca/site/eng/9.500463/publication.html> | Publishing information Ottawa : Office of the Commissioner for Federal Judicial Affairs, [2004]- |
| Latest issue listed on the publications.gc.ca record | 2023 Vol. 3 Part 1 | <https://publications.gc.ca/site/eng/9.500463/publication.html> | 2023;Vol.3;Part.1: JU1-2-1-2023-3-1.pdf |
| vol. 1 | 2004 | <https://publications.gc.ca/site/eng/9.500463/publication.html> | Chronology Vol. 1. Part 1 (2004)- |
| numbering | year_and_volume 2004- | <https://publications.gc.ca/site/eng/9.500463/publication.html> | Chronology Vol. 1. Part 1 (2004)- Frequency 4 volumes a year, |

Not established: Predecessor Federal Court Reports (FC), 1971-2003, holdings (Melbourne guide) 1971-2003.

### CR (cr)

1946 start from the Slaw article heading; end of first series (before the New Series) not established by a fetched page.

| Fact | Value | Source | Quote |
|---|---|---|---|
| Criminal Reports begun | 1946 | <https://www.slaw.ca/2010/09/19/cr-ccc-crcc/> | Criminal Reports: Born 1946 |

### CLRBR (2d) (mcgill-clrbr)

Second series start not established.

Not established: McMaster holdings range for C.L.R.B.R. (holdings only) 1974-2019.

### CRR (2d) (crr)

First series 1982-1991 (50 vols); second series from 1991, continuous volumes. End/current of 2d series not established.

| Fact | Value | Source | Quote |
|---|---|---|---|
| First series CRR | Vol. 1 (1982)-v. 50 (1991) | <https://ci.nii.ac.jp/ncid/AA10849788?l=en> | Canadian rights reporter Butterworth (Canada), c1982-c1991 Vol. 1 (1982)-v. 50 (1991) |
| vol. 1 | 1991 | <https://ci.nii.ac.jp/ncid/AA10849551?l=en> | Vol. 1 (1991) |
| vol. 254 | 2012 | <https://ci.nii.ac.jp/ncid/AA10849551?l=en> | Imprint varies: Markham, Ont. : LexisNexis Canada, <v. 254 (2012)>- |
| numbering | volume 1991- | <https://ci.nii.ac.jp/ncid/AA10849551?l=en> | Canadian rights reporter. Second series Butterworths, c1991- Vol. 1 (1991)- |

### WCB (2d) (wcb)

Second series start not established.

Not established: McMaster online coverage of W.C.B. (coverage, not run) 1977-present.

### WAC (wac)

Nothing found for Western Appeal Cases.

