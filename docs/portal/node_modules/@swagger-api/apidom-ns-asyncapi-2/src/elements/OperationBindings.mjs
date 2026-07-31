import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class OperationBindings extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationBindings';
  }
  get http() {
    return this.get('http');
  }
  set http(http) {
    this.set('http', http);
  }
  get ws() {
    return this.get('ws');
  }
  set ws(ws) {
    this.set('ws', ws);
  }
  get kafka() {
    return this.get('kafka');
  }
  set kafka(kafka) {
    this.set('kafka', kafka);
  }
  get anypointmq() {
    return this.get('anypointmq');
  }
  set anypointmq(anypointmq) {
    this.set('anypointmq', anypointmq);
  }
  get amqp() {
    return this.get('amqp');
  }
  set amqp(amqp) {
    this.set('amqp', amqp);
  }
  get amqp1() {
    return this.get('amqp1');
  }
  set amqp1(amqp1) {
    this.set('amqp1', amqp1);
  }
  get mqtt() {
    return this.get('mqtt');
  }
  set mqtt(mqtt) {
    this.set('mqtt', mqtt);
  }
  get mqtt5() {
    return this.get('mqtt5');
  }
  set mqtt5(mqtt5) {
    this.set('mqtt5', mqtt5);
  }
  get nats() {
    return this.get('nats');
  }
  set nats(nats) {
    this.set('nats', nats);
  }
  get jms() {
    return this.get('jms');
  }
  set jms(jms) {
    this.set('jms', jms);
  }
  get sns() {
    return this.get('sns');
  }
  set sns(sns) {
    this.set('sns', sns);
  }
  get solace() {
    return this.get('solace');
  }
  set solace(solace) {
    this.set('solace', solace);
  }
  get sqs() {
    return this.get('sqs');
  }
  set sqs(sqs) {
    this.set('sqs', sqs);
  }
  get stomp() {
    return this.get('stomp');
  }
  set stomp(stomp) {
    this.set('stomp', stomp);
  }
  get redis() {
    return this.get('redis');
  }
  set redis(redis) {
    this.set('redis', redis);
  }
  get mercure() {
    return this.get('mercure');
  }
  set mercure(mercure) {
    this.set('mercure', mercure);
  }
  get googlepubsub() {
    return this.get('googlepubsub');
  }
  set googlepubsub(googlepubsub) {
    this.set('googlepubsub', googlepubsub);
  }
  get ibmmq() {
    return this.get('ibmmq');
  }
  set ibmmq(ibmmq) {
    this.set('ibmmq', ibmmq);
  }
}
export default OperationBindings;