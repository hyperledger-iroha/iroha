---
lang: my
direction: ltr
source: generatediso20022.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c941de1d2c689a519c2745d4cd1158a1436dc7653e17b4fb928ecbe82cc56b11
source_last_modified: "2025-12-30T13:42:33.041328+00:00"
translation_last_reviewed: 2026-02-07
---

# ISO 20022 Generated Samples

```xml
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <Fr><FIId><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></Fr>
      <To><FIId><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></To>
      <BizMsgIdr>ISO-SAMPLE-001</BizMsgIdr>
      <MsgDefIdr>pacs.008.001.08</MsgDefIdr>
      <BizSvc>IPS</BizSvc>
      <CreDt>2025-11-11T09:34:09Z</CreDt>
    </AppHdr>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
      <FIToFICstmrCdtTrf>
        <GrpHdr>
          <MsgId>ISO-SAMPLE-001</MsgId>
          <CreDtTm>2025-11-11T09:34:09</CreDtTm>
          <BtchBookg>false</BtchBookg>
          <NbOfTxs>1</NbOfTxs>
          <SttlmInf><SttlmMtd>CLRG</SttlmMtd></SttlmInf>
        </GrpHdr>
        <CdtTrfTxInf>
          <PmtId>
            <InstrId>SAMPLE-INSTR-008</InstrId>
            <EndToEndId>SAMPLE-E2E-008</EndToEndId>
            <TxId>SAMPLE-TX-008</TxId>
            <UETR>123e4567-e89b-12d3-a456-426614174000</UETR>
          </PmtId>
          <PmtTpInf>
            <ClrChanl>RTNS</ClrChanl>
            <SvcLvl><Prtry>0100</Prtry></SvcLvl>
            <LclInstrm><Prtry>CTAA</Prtry></LclInstrm>
            <CtgyPurp><Prtry>005</Prtry></CtgyPurp>
          </PmtTpInf>
          <IntrBkSttlmAmt Ccy="USD">1400.00</IntrBkSttlmAmt>
          <IntrBkSttlmDt>2025-11-11</IntrBkSttlmDt>
          <ChrgBr>SHAR</ChrgBr>
          <Dbtr><Nm>Example Debtor</Nm></Dbtr>
          <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
          <DbtrAgt><FinInstnId><BICFI>ALPHGB2L</BICFI></FinInstnId></DbtrAgt>
          <CdtrAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGGB2L</MmbId></ClrSysMmbId></FinInstnId></CdtrAgt>
          <Cdtr><Nm>Example Creditor</Nm></Cdtr>
          <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
          <InstrForNxtAgt><InstrInf>/INFO/Example note</InstrInf></InstrForNxtAgt>
        </CdtTrfTxInf>
      </FIToFICstmrCdtTrf>
    </Document>
  </Body>
</DataEnvelope>
```
<!-- pacs.008.001.08 -->

```xml
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.004.001.09">
      <PmtRtr>
        <GrpHdr>
          <MsgId>ISO-SAMPLE-004</MsgId>
          <CreDtTm>2025-11-06T12:42:25.758Z</CreDtTm>
          <BtchBookg>false</BtchBookg>
          <NbOfTxs>1</NbOfTxs>
          <SttlmInf><SttlmMtd>CLRG</SttlmMtd></SttlmInf>
          <InstdAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></InstdAgt>
        </GrpHdr>
        <OrgnlGrpInf>
          <OrgnlMsgId>ISO-SAMPLE-001</OrgnlMsgId>
          <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
          <OrgnlCreDtTm>2025-11-06T12:42:07.770Z</OrgnlCreDtTm>
        </OrgnlGrpInf>
        <TxInf>
          <RtrId>RETURN-5310</RtrId>
          <OrgnlInstrId>ORIGINAL-INSTR-008</OrgnlInstrId>
          <OrgnlEndToEndId>ORIGINAL-E2E-008</OrgnlEndToEndId>
          <OrgnlTxId>ORIGINAL-TX-008</OrgnlTxId>
          <OrgnlIntrBkSttlmDt>2025-11-06</OrgnlIntrBkSttlmDt>
          <RtrdIntrBkSttlmAmt Ccy="USD">10.00</RtrdIntrBkSttlmAmt>
          <IntrBkSttlmDt>2025-11-06</IntrBkSttlmDt>
          <ChrgBr>SHAR</ChrgBr>
          <InstgAgt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></InstgAgt>
          <InstdAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></InstdAgt>
          <RtrRsnInf><Rsn><Prtry>PR01</Prtry></Rsn></RtrRsnInf>
          <OrgnlTxRef>
            <IntrBkSttlmAmt Ccy="USD">10.00</IntrBkSttlmAmt>
            <PmtTpInf>
              <ClrChanl>RTNS</ClrChanl>
              <SvcLvl><Prtry>0100</Prtry></SvcLvl>
              <LclInstrm><Prtry>CTAA</Prtry></LclInstrm>
              <CtgyPurp><Prtry>001</Prtry></CtgyPurp>
            </PmtTpInf>
            <Dbtr><Pty><Nm>Debtor One</Nm></Pty></Dbtr>
            <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
            <DbtrAgt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></DbtrAgt>
            <CdtrAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></CdtrAgt>
            <Cdtr><Pty><Nm>Creditor One</Nm></Pty></Cdtr>
            <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
          </OrgnlTxRef>
        </TxInf>
      </PmtRtr>
    </Document>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <Fr><FIId><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></Fr>
      <To><FIId><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></To>
      <BizMsgIdr>ISO-SAMPLE-004</BizMsgIdr>
      <MsgDefIdr>pacs.004.001.09</MsgDefIdr>
      <BizSvc>IPS</BizSvc>
      <CreDt>2025-11-06T07:42:25.771Z</CreDt>
      <Sgntr>
        <SignedInfo><SignatureMethod>sample</SignatureMethod></SignedInfo>
        <SignatureValue>ignored</SignatureValue>
      </Sgntr>
    </AppHdr>
  </Body>
</DataEnvelope>
```
<!-- pacs.004.001.09 -->

```xml
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">
  <FIToFIPmtStsRpt>
    <GrpHdr>
      <MsgId>ISO-PACS002-STATUS</MsgId>
      <CreDtTm>2025-11-11T09:35:15.671Z</CreDtTm>
    </GrpHdr>
    <OrgnlGrpInfAndSts>
      <OrgnlMsgId>ISO-SAMPLE-001</OrgnlMsgId>
      <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
      <OrgnlCreDtTm>2025-11-11T09:34:09</OrgnlCreDtTm>
      <OrgnlNbOfTxs>1</OrgnlNbOfTxs>
      <GrpSts>ACSP</GrpSts>
    </OrgnlGrpInfAndSts>
    <TxInfAndSts>
      <StsId>ISO-PACS002-STATUS</StsId>
      <OrgnlInstrId>SAMPLE-INSTR-008</OrgnlInstrId>
      <OrgnlEndToEndId>SAMPLE-E2E-008</OrgnlEndToEndId>
      <OrgnlTxId>SAMPLE-TX-008</OrgnlTxId>
      <TxSts>ACSP</TxSts>
      <AcctSvcrRef>6762</AcctSvcrRef>
      <InstgAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></InstgAgt>
      <InstdAgt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></InstdAgt>
      <OrgnlTxRef>
        <IntrBkSttlmAmt Ccy="USD">1400</IntrBkSttlmAmt>
        <IntrBkSttlmDt>2025-11-11</IntrBkSttlmDt>
        <PmtTpInf>
          <ClrChanl>RTNS</ClrChanl>
          <SvcLvl><Prtry>0100</Prtry></SvcLvl>
          <LclInstrm><Prtry>CTAA</Prtry></LclInstrm>
          <CtgyPurp><Prtry>005</Prtry></CtgyPurp>
        </PmtTpInf>
        <Purp><Prtry>005</Prtry></Purp>
      </OrgnlTxRef>
    </TxInfAndSts>
  </FIToFIPmtStsRpt>
</Document>
```
<!-- pacs.002.001.10 -->

```xml
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08">
  <BkToCstmrAcctRpt>
    <GrpHdr>
      <MsgId>RPT-ALPHA-001</MsgId>
      <CreDtTm>2025-11-10T15:17:49.578Z</CreDtTm>
    </GrpHdr>
    <Rpt>
      <Id>RPT-ALPHA-001</Id>
      <Acct>
        <Id><Othr><Id>ALPHBANK-USD-ACCOUNT-001</Id></Othr></Id>
        <Ccy>USD</Ccy>
      </Acct>
      <Ntry>
        <Amt Ccy="USD">1000.00</Amt>
        <CdtDbtInd>DBIT</CdtDbtInd>
        <BookgDt><Dt>2025-11-10</Dt></BookgDt>
      </Ntry>
    </Rpt>
  </BkToCstmrAcctRpt>
</Document>
```
<!-- camt.052.001.08 -->

```xml
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <Fr><FIId><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></Fr>
      <To><FIId><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></To>
      <BizMsgIdr>ISO-SAMPLE-056-ASGMT-001</BizMsgIdr>
      <MsgDefIdr>camt.056.001.08</MsgDefIdr>
      <BizSvc>IPS</BizSvc>
      <CreDt>2025-11-07T16:27:33Z</CreDt>
      <Sgntr><SignedInfo><SignatureMethod>sample</SignatureMethod></SignedInfo><SignatureValue>ignored</SignatureValue></Sgntr>
    </AppHdr>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.056.001.08">
      <FIToFIPmtCxlReq>
        <Assgnmt>
          <Id>ISO-SAMPLE-056-ASGMT-001</Id>
          <Assgnr><Agt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgnr>
          <Assgne><Agt><FinInstnId><ClrSysMmbId><MmbId>PAYFASTX</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgne>
          <CreDtTm>2025-11-07T16:27:33Z</CreDtTm>
        </Assgnmt>
        <Case>
          <Id>ISO-SAMPLE-056-ASGMT-001</Id>
          <Cretr><Agt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Cretr>
        </Case>
        <Undrlyg>
          <TxInf>
            <OrgnlGrpInf>
              <OrgnlMsgId>ISO-SAMPLE-001</OrgnlMsgId>
              <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
              <OrgnlCreDtTm>2025-11-07T16:13:08.557</OrgnlCreDtTm>
            </OrgnlGrpInf>
            <OrgnlIntrBkSttlmDt>2025-11-07</OrgnlIntrBkSttlmDt>
            <OrgnlUETR>123e4567-e89b-12d3-a456-426614174000</OrgnlUETR>
            <Assgnr><Agt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgnr>
            <Assgne><Agt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgne>
          </TxInf>
        </Undrlyg>
      </FIToFIPmtCxlReq>
    </Document>
  </Body>
</DataEnvelope>
```
<!-- camt.056.001.08 -->
