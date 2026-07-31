import { createRefractor } from "./index.mjs";
/**
 * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
 */
import AsyncApi2Element from "../elements/AsyncApi2.mjs";
import AsyncApiVersionElement from "../elements/AsyncApiVersion.mjs";
import ChannelBindingsElement from "../elements/ChannelBindings.mjs";
import ChannelItemElement from "../elements/ChannelItem.mjs";
import ChannelsElement from "../elements/Channels.mjs";
import ComponentsElement from "../elements/Components.mjs";
import ContactElement from "../elements/Contact.mjs";
import CorrelationIDElement from "../elements/CorrelationID.mjs";
import DefaultContentTypeElement from "../elements/DefaultContentType.mjs";
import ExternalDocumentationElement from "../elements/ExternalDocumentation.mjs";
import IdentifierElement from "../elements/Identifier.mjs";
import InfoElement from "../elements/Info.mjs";
import LicenseElement from "../elements/License.mjs";
import MessageElement from "../elements/Message.mjs";
import MessageBindingsElement from "../elements/MessageBindings.mjs";
import MessageExampleElement from "../elements/MessageExample.mjs";
import MessageTraitElement from "../elements/MessageTrait.mjs";
import OAuthFlowElement from "../elements/OAuthFlow.mjs";
import OAuthFlowsElement from "../elements/OAuthFlows.mjs";
import OperationElement from "../elements/Operation.mjs";
import OperationBindingsElement from "../elements/OperationBindings.mjs";
import OperationTraitElement from "../elements/OperationTrait.mjs";
import ParameterElement from "../elements/Parameter.mjs";
import ParametersElement from "../elements/Parameters.mjs";
import ReferenceElement from "../elements/Reference.mjs";
import SchemaElement from "../elements/Schema.mjs";
import SecurityRequirementElement from "../elements/SecurityRequirement.mjs";
import SecuritySchemeElement from "../elements/SecurityScheme.mjs";
import ServerElement from "../elements/Server.mjs";
import ServerBindingsElement from "../elements/ServerBindings.mjs";
import ServersElement from "../elements/Servers.mjs";
import ServerVariableElement from "../elements/ServerVariable.mjs";
import TagElement from "../elements/Tag.mjs";
import TagsElement from "../elements/Tags.mjs";
/**
 * Binding elements.
 */
// AMQP 0-9-1
import AmqpChannelBindingElement from "../elements/bindings/amqp/AmqpChannelBinding.mjs";
import AmqpMessageBindingElement from "../elements/bindings/amqp/AmqpMessageBinding.mjs";
import AmqpOperationBindingElement from "../elements/bindings/amqp/AmqpOperationBinding.mjs";
import AmqpServerBindingElement from "../elements/bindings/amqp/AmqpServerBinding.mjs"; // AMQP 1.0
import Amqp1ChannelBindingElement from "../elements/bindings/amqp1/Amqp1ChannelBinding.mjs";
import Amqp1MessageBindingElement from "../elements/bindings/amqp1/Amqp1MessageBinding.mjs";
import Amqp1OperationBindingElement from "../elements/bindings/amqp1/Amqp1OperationBinding.mjs";
import Amqp1ServerBindingElement from "../elements/bindings/amqp1/Amqp1ServerBinding.mjs"; // Anypoint MQ
import AnypointmqChannelBindingElement from "../elements/bindings/anypointmq/AnypointmqChannelBinding.mjs";
import AnypointmqMessageBindingElement from "../elements/bindings/anypointmq/AnypointmqMessageBinding.mjs";
import AnypointmqOperationBindingElement from "../elements/bindings/anypointmq/AnypointmqOperationBinding.mjs";
import AnypointmqServerBindingElement from "../elements/bindings/anypointmq/AnypointmqServerBinding.mjs"; // HTTP
import HttpChannelBindingElement from "../elements/bindings/http/HttpChannelBinding.mjs";
import HttpMessageBindingElement from "../elements/bindings/http/HttpMessageBinding.mjs";
import HttpOperationBindingElement from "../elements/bindings/http/HttpOperationBinding.mjs";
import HttpServerBindingElement from "../elements/bindings/http/HttpServerBinding.mjs"; // Google Cloud Pub/Sub
import GooglepubsubChannelBindingElement from "../elements/bindings/googlepubsub/GooglepubsubChannelBinding.mjs";
import GooglepubsubMessageBindingElement from "../elements/bindings/googlepubsub/GooglepubsubMessageBinding.mjs";
import GooglepubsubOperationBindingElement from "../elements/bindings/googlepubsub/GooglepubsubOperationBinding.mjs";
import GooglepubsubServerBindingElement from "../elements/bindings/googlepubsub/GooglepubsubServerBinding.mjs"; // IBM MQ
import IbmmqChannelBindingElement from "../elements/bindings/ibmmq/IbmmqChannelBinding.mjs";
import IbmmqMessageBindingElement from "../elements/bindings/ibmmq/IbmmqMessageBinding.mjs";
import IbmmqOperationBindingElement from "../elements/bindings/ibmmq/IbmmqOperationBinding.mjs";
import IbmmqServerBindingElement from "../elements/bindings/ibmmq/IbmmqServerBinding.mjs"; // JMS
import JmsChannelBindingElement from "../elements/bindings/jms/JmsChannelBinding.mjs";
import JmsMessageBindingElement from "../elements/bindings/jms/JmsMessageBinding.mjs";
import JmsOperationBindingElement from "../elements/bindings/jms/JmsOperationBinding.mjs";
import JmsServerBindingElement from "../elements/bindings/jms/JmsServerBinding.mjs"; // Kafka
import KafkaChannelBindingElement from "../elements/bindings/kafka/KafkaChannelBinding.mjs";
import KafkaMessageBindingElement from "../elements/bindings/kafka/KafkaMessageBinding.mjs";
import KafkaOperationBindingElement from "../elements/bindings/kafka/KafkaOperationBinding.mjs";
import KafkaServerBindingElement from "../elements/bindings/kafka/KafkaServerBinding.mjs"; // Mercure
import MercureChannelBindingElement from "../elements/bindings/mercure/MercureChannelBinding.mjs";
import MercureMessageBindingElement from "../elements/bindings/mercure/MercureMessageBinding.mjs";
import MercureOperationBindingElement from "../elements/bindings/mercure/MercureOperationBinding.mjs";
import MercureServerBindingElement from "../elements/bindings/mercure/MercureServerBinding.mjs"; // MQTT
import MqttChannelBindingElement from "../elements/bindings/mqtt/MqttChannelBinding.mjs";
import MqttMessageBindingElement from "../elements/bindings/mqtt/MqttMessageBinding.mjs";
import MqttOperationBindingElement from "../elements/bindings/mqtt/MqttOperationBinding.mjs";
import MqttServerBindingElement from "../elements/bindings/mqtt/MqttServerBinding.mjs"; // MQTT 5
import Mqtt5ChannelBindingElement from "../elements/bindings/mqtt5/Mqtt5ChannelBinding.mjs";
import Mqtt5MessageBindingElement from "../elements/bindings/mqtt5/Mqtt5MessageBinding.mjs";
import Mqtt5OperationBindingElement from "../elements/bindings/mqtt5/Mqtt5OperationBinding.mjs";
import Mqtt5ServerBindingElement from "../elements/bindings/mqtt5/Mqtt5ServerBinding.mjs"; // NATS
import NatsChannelBindingElement from "../elements/bindings/nats/NatsChannelBinding.mjs";
import NatsMessageBindingElement from "../elements/bindings/nats/NatsMessageBinding.mjs";
import NatsOperationBindingElement from "../elements/bindings/nats/NatsOperationBinding.mjs";
import NatsServerBindingElement from "../elements/bindings/nats/NatsServerBinding.mjs"; // Pulsar
import PulsarChannelBindingElement from "../elements/bindings/pulsar/PulsarChannelBinding.mjs";
import PulsarMessageBindingElement from "../elements/bindings/pulsar/PulsarMessageBinding.mjs";
import PulsarOperationBindingElement from "../elements/bindings/pulsar/PulsarOperationBinding.mjs";
import PulsarServerBindingElement from "../elements/bindings/pulsar/PulsarServerBinding.mjs"; // Redis
import RedisChannelBindingElement from "../elements/bindings/redis/RedisChannelBinding.mjs";
import RedisMessageBindingElement from "../elements/bindings/redis/RedisMessageBinding.mjs";
import RedisOperationBindingElement from "../elements/bindings/redis/RedisOperationBinding.mjs";
import RedisServerBindingElement from "../elements/bindings/redis/RedisServerBinding.mjs"; // SNS
import SnsChannelBindingElement from "../elements/bindings/sns/SnsChannelBinding.mjs";
import SnsMessageBindingElement from "../elements/bindings/sns/SnsMessageBinding.mjs";
import SnsOperationBindingElement from "../elements/bindings/sns/SnsOperationBinding.mjs";
import SnsServerBindingElement from "../elements/bindings/sns/SnsServerBinding.mjs"; // Solace
import SolaceChannelBindingElement from "../elements/bindings/solace/SolaceChannelBinding.mjs";
import SolaceMessageBindingElement from "../elements/bindings/solace/SolaceMessageBinding.mjs";
import SolaceOperationBindingElement from "../elements/bindings/solace/SolaceOperationBinding.mjs";
import SolaceServerBindingElement from "../elements/bindings/solace/SolaceServerBinding.mjs"; // SQS
import SqsChannelBindingElement from "../elements/bindings/sqs/SqsChannelBinding.mjs";
import SqsMessageBindingElement from "../elements/bindings/sqs/SqsMessageBinding.mjs";
import SqsOperationBindingElement from "../elements/bindings/sqs/SqsOperationBinding.mjs";
import SqsServerBindingElement from "../elements/bindings/sqs/SqsServerBinding.mjs"; // STOMP
import StompChannelBindingElement from "../elements/bindings/stomp/StompChannelBinding.mjs";
import StompMessageBindingElement from "../elements/bindings/stomp/StompMessageBinding.mjs";
import StompOperationBindingElement from "../elements/bindings/stomp/StompOperationBinding.mjs";
import StompServerBindingElement from "../elements/bindings/stomp/StompServerBinding.mjs"; // WebSocket
import WebSocketChannelBindingElement from "../elements/bindings/ws/WebSocketChannelBinding.mjs";
import WebSocketMessageBindingElement from "../elements/bindings/ws/WebSocketMessageBinding.mjs";
import WebSocketOperationBindingElement from "../elements/bindings/ws/WebSocketOperationBinding.mjs";
import WebSocketServerBindingElement from "../elements/bindings/ws/WebSocketServerBinding.mjs";
/**
 * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
 */
AsyncApi2Element.refract = createRefractor(['visitors', 'document', 'objects', 'AsyncApi', '$visitor']);
AsyncApiVersionElement.refract = createRefractor(['visitors', 'document', 'objects', 'AsyncApiVersion', '$visitor']);
ChannelBindingsElement.refract = createRefractor(['visitors', 'document', 'objects', 'ChannelBindings', '$visitor']);
ChannelItemElement.refract = createRefractor(['visitors', 'document', 'objects', 'ChannelItem', '$visitor']);
ChannelsElement.refract = createRefractor(['visitors', 'document', 'objects', 'Channels', '$visitor']);
ComponentsElement.refract = createRefractor(['visitors', 'document', 'objects', 'Components', '$visitor']);
ContactElement.refract = createRefractor(['visitors', 'document', 'objects', 'Contact', '$visitor']);
CorrelationIDElement.refract = createRefractor(['visitors', 'document', 'objects', 'CorrelationID', '$visitor']);
DefaultContentTypeElement.refract = createRefractor(['visitors', 'document', 'objects', 'DefaultContentType', '$visitor']);
ExternalDocumentationElement.refract = createRefractor(['visitors', 'document', 'objects', 'ExternalDocumentation', '$visitor']);
IdentifierElement.refract = createRefractor(['visitors', 'document', 'objects', 'Identifier', '$visitor']);
InfoElement.refract = createRefractor(['visitors', 'document', 'objects', 'Info', '$visitor']);
LicenseElement.refract = createRefractor(['visitors', 'document', 'objects', 'License', '$visitor']);
MessageElement.refract = createRefractor(['visitors', 'document', 'objects', 'Message', '$visitor']);
MessageBindingsElement.refract = createRefractor(['visitors', 'document', 'objects', 'MessageBindings', '$visitor']);
MessageExampleElement.refract = createRefractor(['visitors', 'document', 'objects', 'MessageExample', '$visitor']);
MessageTraitElement.refract = createRefractor(['visitors', 'document', 'objects', 'MessageTrait', '$visitor']);
OAuthFlowElement.refract = createRefractor(['visitors', 'document', 'objects', 'OAuthFlow', '$visitor']);
OAuthFlowsElement.refract = createRefractor(['visitors', 'document', 'objects', 'OAuthFlows', '$visitor']);
OperationElement.refract = createRefractor(['visitors', 'document', 'objects', 'Operation', '$visitor']);
OperationBindingsElement.refract = createRefractor(['visitors', 'document', 'objects', 'OperationBindings', '$visitor']);
OperationTraitElement.refract = createRefractor(['visitors', 'document', 'objects', 'OperationTrait', '$visitor']);
ParameterElement.refract = createRefractor(['visitors', 'document', 'objects', 'Parameter', '$visitor']);
ParametersElement.refract = createRefractor(['visitors', 'document', 'objects', 'Parameters', '$visitor']);
ReferenceElement.refract = createRefractor(['visitors', 'document', 'objects', 'Reference', '$visitor']);
SchemaElement.refract = createRefractor(['visitors', 'document', 'objects', 'Schema', '$visitor']);
SecurityRequirementElement.refract = createRefractor(['visitors', 'document', 'objects', 'SecurityRequirement', '$visitor']);
SecuritySchemeElement.refract = createRefractor(['visitors', 'document', 'objects', 'SecurityScheme', '$visitor']);
ServerElement.refract = createRefractor(['visitors', 'document', 'objects', 'Server', '$visitor']);
ServerBindingsElement.refract = createRefractor(['visitors', 'document', 'objects', 'ServerBindings', '$visitor']);
ServersElement.refract = createRefractor(['visitors', 'document', 'objects', 'Servers', '$visitor']);
ServerVariableElement.refract = createRefractor(['visitors', 'document', 'objects', 'ServerVariable', '$visitor']);
TagElement.refract = createRefractor(['visitors', 'document', 'objects', 'Tag', '$visitor']);
TagsElement.refract = createRefractor(['visitors', 'document', 'objects', 'Tags', '$visitor']);

/**
 * Binding elements.
 */
// AMQP 0-9-1
AmqpChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp', 'ChannelBinding', '$visitor']);
AmqpMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp', 'MessageBinding', '$visitor']);
AmqpOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp', 'OperationBinding', '$visitor']);
AmqpServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp', 'ServerBinding', '$visitor']);
// AMQP 1.0
Amqp1ChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp1', 'ChannelBinding', '$visitor']);
Amqp1MessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp1', 'MessageBinding', '$visitor']);
Amqp1OperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp1', 'OperationBinding', '$visitor']);
Amqp1ServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'amqp1', 'ServerBinding', '$visitor']);
// HTTP
HttpChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'http', 'ChannelBinding', '$visitor']);
HttpMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'http', 'MessageBinding', '$visitor']);
HttpOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'http', 'OperationBinding', '$visitor']);
HttpServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'http', 'ServerBinding', '$visitor']);
// Google Cloud Pub/Sub
GooglepubsubChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'googlepubsub', 'ChannelBinding', '$visitor']);
GooglepubsubMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'googlepubsub', 'MessageBinding', '$visitor']);
GooglepubsubOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'googlepubsub', 'OperationBinding', '$visitor']);
GooglepubsubServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'googlepubsub', 'ServerBinding', '$visitor']);
// IBM MQ
IbmmqChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ibmmq', 'ChannelBinding', '$visitor']);
IbmmqMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ibmmq', 'MessageBinding', '$visitor']);
IbmmqOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ibmmq', 'OperationBinding', '$visitor']);
IbmmqServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ibmmq', 'ServerBinding', '$visitor']);
// JMS
JmsChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'jms', 'ChannelBinding', '$visitor']);
JmsMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'jms', 'MessageBinding', '$visitor']);
JmsOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'jms', 'OperationBinding', '$visitor']);
JmsServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'jms', 'ServerBinding', '$visitor']);
// Kafka
KafkaChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'kafka', 'ChannelBinding', '$visitor']);
KafkaMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'kafka', 'MessageBinding', '$visitor']);
KafkaOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'kafka', 'OperationBinding', '$visitor']);
KafkaServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'kafka', 'ServerBinding', '$visitor']);
// Anypoint MQ
AnypointmqChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'anypointmq', 'ChannelBinding', '$visitor']);
AnypointmqMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'anypointmq', 'MessageBinding', '$visitor']);
AnypointmqOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'anypointmq', 'OperationBinding', '$visitor']);
AnypointmqServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'anypointmq', 'ServerBinding', '$visitor']);
// Mercure
MercureChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mercure', 'ChannelBinding', '$visitor']);
MercureMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mercure', 'MessageBinding', '$visitor']);
MercureOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mercure', 'OperationBinding', '$visitor']);
MercureServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mercure', 'ServerBinding', '$visitor']);
// MQTT
MqttChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt', 'ChannelBinding', '$visitor']);
MqttMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt', 'MessageBinding', '$visitor']);
MqttOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt', 'OperationBinding', '$visitor']);
MqttServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt', 'ServerBinding', '$visitor']);
// MQTT 5
Mqtt5ChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt5', 'ChannelBinding', '$visitor']);
Mqtt5MessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt5', 'MessageBinding', '$visitor']);
Mqtt5OperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt5', 'OperationBinding', '$visitor']);
Mqtt5ServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'mqtt5', 'ServerBinding', '$visitor']);
// NATS
NatsChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'nats', 'ChannelBinding', '$visitor']);
NatsMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'nats', 'MessageBinding', '$visitor']);
NatsOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'nats', 'OperationBinding', '$visitor']);
NatsServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'nats', 'ServerBinding', '$visitor']);
// Pulsar
PulsarChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'pulsar', 'ChannelBinding', '$visitor']);
PulsarMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'pulsar', 'MessageBinding', '$visitor']);
PulsarOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'pulsar', 'OperationBinding', '$visitor']);
PulsarServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'pulsar', 'ServerBinding', '$visitor']);
// Redis
RedisChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'redis', 'ChannelBinding', '$visitor']);
RedisMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'redis', 'MessageBinding', '$visitor']);
RedisOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'redis', 'OperationBinding', '$visitor']);
RedisServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'redis', 'ServerBinding', '$visitor']);
// SNS
SnsChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sns', 'ChannelBinding', '$visitor']);
SnsMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sns', 'MessageBinding', '$visitor']);
SnsOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sns', 'OperationBinding', '$visitor']);
SnsServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sns', 'ServerBinding', '$visitor']);
// Solace
SolaceChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'solace', 'ChannelBinding', '$visitor']);
SolaceMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'solace', 'MessageBinding', '$visitor']);
SolaceOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'solace', 'OperationBinding', '$visitor']);
SolaceServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'solace', 'ServerBinding', '$visitor']);
// SQS
SqsChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sqs', 'ChannelBinding', '$visitor']);
SqsMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sqs', 'MessageBinding', '$visitor']);
SqsOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sqs', 'OperationBinding', '$visitor']);
SqsServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'sqs', 'ServerBinding', '$visitor']);
// STOMP
StompChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'stomp', 'ChannelBinding', '$visitor']);
StompMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'stomp', 'MessageBinding', '$visitor']);
StompOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'stomp', 'OperationBinding', '$visitor']);
StompServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'stomp', 'ServerBinding', '$visitor']);
// WebSocket
WebSocketChannelBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ws', 'ChannelBinding', '$visitor']);
WebSocketMessageBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ws', 'MessageBinding', '$visitor']);
WebSocketOperationBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ws', 'OperationBinding', '$visitor']);
WebSocketServerBindingElement.refract = createRefractor(['visitors', 'document', 'objects', 'bindings', 'ws', 'ServerBinding', '$visitor']);
export {
/**
 * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
 */
AsyncApi2Element, AsyncApiVersionElement, ChannelBindingsElement, ChannelItemElement, ChannelsElement, ComponentsElement, ContactElement, CorrelationIDElement, DefaultContentTypeElement, ExternalDocumentationElement, IdentifierElement, InfoElement, LicenseElement, MessageElement, MessageBindingsElement, MessageExampleElement, MessageTraitElement, OAuthFlowElement, OAuthFlowsElement, OperationElement, OperationBindingsElement, OperationTraitElement, ParameterElement, ParametersElement, ReferenceElement, SchemaElement, SecurityRequirementElement, SecuritySchemeElement, ServerElement, ServerBindingsElement, ServersElement, ServerVariableElement, TagElement, TagsElement,
/**
 * Binding elements.
 */
// AMQP 0-9-1
AmqpChannelBindingElement, AmqpMessageBindingElement, AmqpOperationBindingElement, AmqpServerBindingElement,
// AMQP 1.0
Amqp1ChannelBindingElement, Amqp1MessageBindingElement, Amqp1OperationBindingElement, Amqp1ServerBindingElement,
// Anypoint MQ
AnypointmqChannelBindingElement, AnypointmqMessageBindingElement, AnypointmqOperationBindingElement, AnypointmqServerBindingElement,
// Google Cloud Pub/Sub
GooglepubsubChannelBindingElement, GooglepubsubMessageBindingElement, GooglepubsubOperationBindingElement, GooglepubsubServerBindingElement,
// HTTP
HttpChannelBindingElement, HttpMessageBindingElement, HttpOperationBindingElement, HttpServerBindingElement,
// IBM MQ
IbmmqChannelBindingElement, IbmmqMessageBindingElement, IbmmqOperationBindingElement, IbmmqServerBindingElement,
// JMS
JmsChannelBindingElement, JmsMessageBindingElement, JmsOperationBindingElement, JmsServerBindingElement,
// Kafka
KafkaChannelBindingElement, KafkaMessageBindingElement, KafkaOperationBindingElement, KafkaServerBindingElement,
// Mercure
MercureChannelBindingElement, MercureMessageBindingElement, MercureOperationBindingElement, MercureServerBindingElement,
// MQTT
MqttChannelBindingElement, MqttMessageBindingElement, MqttOperationBindingElement, MqttServerBindingElement,
// MQTT 5
Mqtt5ChannelBindingElement, Mqtt5MessageBindingElement, Mqtt5OperationBindingElement, Mqtt5ServerBindingElement,
// NATS
NatsChannelBindingElement, NatsMessageBindingElement, NatsOperationBindingElement, NatsServerBindingElement,
// Pulsar
PulsarChannelBindingElement, PulsarMessageBindingElement, PulsarOperationBindingElement, PulsarServerBindingElement,
// Redis
RedisChannelBindingElement, RedisMessageBindingElement, RedisOperationBindingElement, RedisServerBindingElement,
// SNS
SnsChannelBindingElement, SnsMessageBindingElement, SnsOperationBindingElement, SnsServerBindingElement,
// Solace
SolaceChannelBindingElement, SolaceMessageBindingElement, SolaceOperationBindingElement, SolaceServerBindingElement,
// SQS
SqsChannelBindingElement, SqsMessageBindingElement, SqsOperationBindingElement, SqsServerBindingElement,
// STOMP
StompChannelBindingElement, StompMessageBindingElement, StompOperationBindingElement, StompServerBindingElement,
// WebSocket
WebSocketChannelBindingElement, WebSocketMessageBindingElement, WebSocketOperationBindingElement, WebSocketServerBindingElement };