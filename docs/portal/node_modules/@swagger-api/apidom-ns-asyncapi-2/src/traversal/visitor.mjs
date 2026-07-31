import { isElement, keyMap as keyMapBase } from '@swagger-api/apidom-core';

/**
 * @public
 */
export const getNodeType = element => {
  if (!isElement(element)) {
    return undefined;
  }
  return `${element.element.charAt(0).toUpperCase() + element.element.slice(1)}Element`;
};

/**
 * @public
 */
export const keyMap = {
  /**
   * `AsyncApi >= 2.0.0 <=2.6.0` specification elements.
   */
  AsyncApi2Element: ['content'],
  AsyncApiVersionElement: [],
  ChannelBindingsElement: ['content'],
  ChannelItemElement: ['content'],
  ChannelsElement: ['content'],
  ComponentsElement: ['content'],
  ContactElement: ['content'],
  CorrelationIDElement: ['content'],
  DefaultContentType: [],
  ExternalDocumentationElement: ['content'],
  InfoElement: ['content'],
  LicenseElement: ['content'],
  MessageElement: ['content'],
  MessageBindingsElement: ['content'],
  MessageTraitElement: ['content'],
  OAuthFlowElement: ['content'],
  OAuthFlowsElement: ['content'],
  OperationElement: ['content'],
  OperationBindingsElement: ['content'],
  OperationTraitElement: ['content'],
  ParameterElement: ['content'],
  ParametersElement: ['content'],
  ReferenceElement: ['content'],
  SchemaElement: ['content'],
  SecurityRequirementElement: ['content'],
  SecuritySchemeElement: ['content'],
  ServerElement: ['content'],
  ServerBindingsElement: ['content'],
  ServersElement: ['content'],
  ServerVariableElement: ['content'],
  TagElement: ['content'],
  TagsElement: ['content'],
  /**
   * Binding elements.
   */
  // AMQP 0-9-1
  AmqpChannelBindingElement: ['content'],
  AmqpMessageBindingElement: ['content'],
  AmqpOperationBindingElement: ['content'],
  AmqpServerBindingElement: ['content'],
  // AMQP 1.0
  Amqp1ChannelBindingElement: ['content'],
  Amqp1MessageBindingElement: ['content'],
  Amqp1OperationBindingElement: ['content'],
  Amqp1ServerBindingElement: ['content'],
  // Anypoint MQ
  AnypointmqChannelBindingElement: ['content'],
  AnypointmqMessageBindingElement: ['content'],
  AnypointmqOperationBindingElement: ['content'],
  AnypointmqServerBindingElement: ['content'],
  // Google Cloud Pub/Sub Server Binding
  GooglepubsubChannelBindingElement: ['content'],
  GooglepubsubMessageBindingElement: ['content'],
  GooglepubsubOperationBindingElement: ['content'],
  GooglepubsubServerBindingElement: ['content'],
  // HTTP
  HttpChannelBindingElement: ['content'],
  HttpMessageBindingElement: ['content'],
  HttpOperationBindingElement: ['content'],
  HttpServerBindingElement: ['content'],
  // IBM MQ
  IbmmqChannelBindingElement: ['content'],
  IbmmqMessageBindingElement: ['content'],
  IbmmqChannelOperationElement: ['content'],
  IbmmqServerBindingElement: ['content'],
  // JMS
  JmsChannelBindingElement: ['content'],
  JmsMessageBindingElement: ['content'],
  JmsOperationBindingElement: ['content'],
  JmsServerBindingElement: ['content'],
  // Kafka
  KafkaChannelBindingElement: ['content'],
  KafkaMessageBindingElement: ['content'],
  KafkaOperationBindingElement: ['content'],
  KafkaServerBindingElement: ['content'],
  // Mercure
  MercureChannelBindingElement: ['content'],
  MercureMessageBindingElement: ['content'],
  MercureOperationBindingElement: ['content'],
  MercureServerBindingElement: ['content'],
  // MQTT
  MqttChannelBindingElement: ['content'],
  MqttMessageBindingElement: ['content'],
  MqttOperationBindingElement: ['content'],
  MqttServerBindingElement: ['content'],
  // MQTT 5
  Mqtt5ChannelBindingElement: ['content'],
  Mqtt5MessageBindingElement: ['content'],
  Mqtt5OperationBindingElement: ['content'],
  Mqtt5ServerBindingElement: ['content'],
  // NATS
  NatsChannelBindingElement: ['content'],
  NatsMessageBindingElement: ['content'],
  NatsOperationBindingElement: ['content'],
  NatsServerBindingElement: ['content'],
  // Pulsar
  PulsarChannelBindingElement: ['content'],
  PulsarMessageBindingElement: ['content'],
  PulsarOperationBindingElement: ['content'],
  PulsarServerBindingElement: ['content'],
  // Redis
  RedisChannelBindingElement: ['content'],
  RedisMessageBindingElement: ['content'],
  RedisOperationBindingElement: ['content'],
  RedisServerBindingElement: ['content'],
  // SNS
  SnsChannelBindingElement: ['content'],
  SnsMessageBindingElement: ['content'],
  SnsOperationBindingElement: ['content'],
  SnsServerBindingElement: ['content'],
  // Solace
  SolaceChannelBindingElement: ['content'],
  SolaceMessageBindingElement: ['content'],
  SolaceOperationBindingElement: ['content'],
  SolaceServerBindingElement: ['content'],
  // SQS
  SqsChannelBindingElement: ['content'],
  SqsMessageBindingElement: ['content'],
  SqsOperationBindingElement: ['content'],
  SqsServerBindingElement: ['content'],
  // STOMP
  StompChannelBindingElement: ['content'],
  StompMessageBindingElement: ['content'],
  StompOperationBindingElement: ['content'],
  StompServerBindingElement: ['content'],
  // WebSocket
  WebSocketChannelBindingElement: ['content'],
  WebSocketMessageBindingElement: ['content'],
  WebSocketOperationBindingElement: ['content'],
  WebSocketServerBindingElement: ['content'],
  ...keyMapBase
};